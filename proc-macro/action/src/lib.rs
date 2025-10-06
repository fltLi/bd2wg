extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Attribute, Data, DeriveInput, Fields, Ident, Lit, Meta, NestedMeta, Type, parse_macro_input,
};

/// Actionable 派生宏 (derive macro) 实现文件
///
/// 这个 proc-macro 提供了一个名为 `Actionable` 的派生宏，它会为标注了该宏的结构体生成若干 trait/impl：
/// - 为结构体生成 `Display` 实现，用于把结构体序列化成命令行风格的字符串（head + main + args）
/// - 为结构体生成 `Into<Action>` 的实现（把结构体打包为通用的 `Action`）
/// - 为结构体生成 `Actionable` marker 实现，保证类型满足框架中对 Actionable 的约束
/// - 当结构体未标注 `#[action(custom)]` 时，会自动生成一个空的 `ActionCustom` impl；
///   如果标注了 `#[action(custom)]`，则表示用户会自己实现 `ActionCustom`，宏不会自动生成。
///
/// 支持的结构体级别属性（通过 `#[action(...)]`）：
/// - head = "..."  : 指定固定的 head 字符串（如果未提供，则在运行时调用 `get_head()`）
/// - main = "single" | "list" : 指定 main 部分的类型；当指定时，必须在某个字段上使用 `#[action(main)]` 标记
/// - custom : 当存在时，表示用户提供自定义的 `ActionCustom` 实现，宏不会自动生成默认 impl
///
/// 支持的字段级别属性（通过 `#[action(...)]`）：
/// - main : 把该字段作为 main 部分（配合结构体的 `main` 设置使用）
/// - nullable : 该字段是可选的（通常与 Option<T> 一起使用），在生成 args 时会以是否为 Some 判定
/// - none : 与 `nullable` 一起使用；当字段类型为 `Option<T>` 且值为 `None` 时，会生成 `none` 文本（如 `-name=none` 或 `-name=none`），否则默认跳过
/// - arg = "tag" | "pair" | "value" : 指定该字段如何生成命令行参数
///     - tag: 生成 `-name` （布尔类型常用）
///     - pair: 生成 `-name=value`
///     - value: 生成 `-value`（不带字段名）
/// - rename = "xxx" : 生成参数时使用的名字（覆盖字段名）
/// - tie = "other" : 当 tag 为 true 时同时推入一个附加的 `-other`（用于关联开关）
/// - none : 与 `nullable` 一起使用，当字段为 `Option<T>` 且值为 `None` 时，使用字符串 `none` 作为参数值而不是忽略该字段
///
/// 生成规则要点：
/// - head 与 main 两部分优先使用属性指定的静态值；否则分别调用 `get_head()` 和 `get_main()`
/// - args 部分会根据字段的 `arg`/`nullable`/`rename`/`tie` 等信息动态构建
/// - 对于 Option<T> 自动识别为可选类型（is_option_type 函数），并在生成 args 时以 `if let Some(...)` 包裹
///
/// 简单示例：
/// ```ignore
/// #[derive(Actionable)]
/// #[action(head = "browse", main = "list")]
/// struct Browse {
///     #[action(main)] items: Vec<String>,
///     #[action(arg = "tag", rename = "force")] force: bool
/// }
/// ```
///
/// 上述示例会将实例格式化为类似 "browse:item1|item2 -force" 的字符串（具体由字段值决定）。
#[proc_macro_derive(Actionable, attributes(action))]
pub fn derive_actionable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    // 解析结构体属性
    let struct_attrs = parse_struct_attrs(&input.attrs);

    // 确保是命名结构体
    let fields = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => fields.named,
            _ => panic!("Only structs with named fields are supported"),
        },
        _ => panic!("Only structs are supported"),
    };

    // 解析字段信息
    let field_infos: Vec<_> = fields.into_iter().map(parse_field_attrs).collect();

    // 检查是否需要生成空的 ActionCustom 实现：
    // 新行为：当未标注 #[action(custom)] 时自动生成默认 impl；如果标注了 #[action(custom)] 则不生成。
    let custom_impl = if struct_attrs.custom {
        quote! {}
    } else {
        generate_action_custom_impl(&name)
    };

    // 生成对 Actionable 特型的实现，确保使用此派生的结构体都实现 Actionable
    let actionable_impl = generate_actionable_impl(&name);

    let into_action_impl = generate_into_action_impl(&name);

    // 生成 display 实现
    let display_impl = generate_display_impl(&struct_attrs, &field_infos, &name);

    let expanded = quote! {
        #custom_impl
        #display_impl
        #actionable_impl
        #into_action_impl
    };

    TokenStream::from(expanded)
}

/// 解析结构体属性
/// 解析结构体上 `#[action(...)]` 属性并返回 `StructAttrs`。
///
/// 支持的键和值在文件顶部的注释中有说明。函数会遍历每个 attribute，寻找 path 为 `action` 的属性，
/// 并从里面解析 `head`, `main`, `no_custom` 三类选项。
fn parse_struct_attrs(attrs: &[Attribute]) -> StructAttrs {
    let mut head = None;
    let mut main = None;
    // 当结构体标注 #[action(custom)] 时，表示用户提供自定义 ActionCustom 实现
    let mut custom = false;

    for attr in attrs {
        if attr.path.is_ident("action")
            && let Ok(Meta::List(meta_list)) = attr.parse_meta()
        {
            for nested in meta_list.nested {
                if let NestedMeta::Meta(meta) = nested {
                    match meta {
                        Meta::NameValue(nv) => {
                            if nv.path.is_ident("head") {
                                if let Lit::Str(lit) = nv.lit {
                                    head = Some(lit.value());
                                }
                            } else if nv.path.is_ident("main")
                                && let Lit::Str(lit) = nv.lit
                            {
                                main = Some(lit.value());
                            }
                        }
                        Meta::Path(path) => {
                            if path.is_ident("custom") {
                                custom = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    StructAttrs { head, main, custom }
}

// 结构体属性
struct StructAttrs {
    head: Option<String>,
    main: Option<String>,
    // true 表示用户实现了自定义 ActionCustom，宏不应自动生成默认 impl
    custom: bool,
}

// 解析字段属性
/// 解析单个字段的 `#[action(...)]` 属性，生成 `FieldInfo`。
///
/// 返回的 `FieldInfo` 包含：字段名标识、类型、是否是 main 字段、arg 类型（tag/pair/value）、
/// rename 覆盖名、tie 绑定名、以及 nullable 标志。
fn parse_field_attrs(field: syn::Field) -> FieldInfo {
    let ident = field.ident.clone().expect("Field must have identifier");
    let ty = field.ty;
    // 字段标记
    let mut main = false;
    // arg 类型：tag | pair | value
    let mut arg = None;
    // 参数重命名
    let mut rename = None;
    // tie: 关联开关名
    let mut tie = None;
    // none 标志：当字段为 Option 且为 None 时，显式输出 `none` 而不是跳过
    let mut none = false;
    // nullable 标志（表示字段可能为 None）
    let mut nullable = false;

    for attr in field.attrs {
        // 我们只关心 path 为 `action` 的属性
        if attr.path.is_ident("action")
            && let Ok(Meta::List(meta_list)) = attr.parse_meta()
        {
            for nested in meta_list.nested {
                if let NestedMeta::Meta(meta) = nested {
                    match meta {
                        Meta::Path(path) => {
                            // 单纯的标记，如 `main` 或 `nullable`
                            if path.is_ident("main") {
                                main = true;
                            } else if path.is_ident("nullable") {
                                nullable = true;
                            } else if path.is_ident("none") {
                                none = true;
                            }
                        }
                        Meta::NameValue(nv) => {
                            // 键值对形式，如 arg = "tag" / rename = "xxx" / tie = "xxx"
                            if nv.path.is_ident("arg") {
                                if let Lit::Str(lit) = nv.lit {
                                    arg = Some(lit.value());
                                }
                            } else if nv.path.is_ident("rename") {
                                if let Lit::Str(lit) = nv.lit {
                                    rename = Some(lit.value());
                                }
                            } else if nv.path.is_ident("tie")
                                && let Lit::Str(lit) = nv.lit
                            {
                                tie = Some(lit.value());
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // 禁止 arg = "tag" 与 none 同时使用（tag 无法有值语义）
    if none
        && let Some(a) = &arg
        && a == "tag"
    {
        panic!("#[action(none)] cannot be used with #[action(arg = \"tag\")] on the same field");
    }

    FieldInfo {
        ident,
        ty,
        main,
        arg,
        rename,
        tie,
        none,
        nullable,
    }
}

// 字段信息
struct FieldInfo {
    ident: Ident,
    ty: syn::Type,
    main: bool,
    arg: Option<String>,
    rename: Option<String>,
    tie: Option<String>,
    none: bool,
    nullable: bool,
}

// 生成 ActionCustom 实现
fn generate_action_custom_impl(name: &Ident) -> proc_macro2::TokenStream {
    quote! {
        impl ActionCustom for #name {}
    }
}

fn generate_into_action_impl(name: &Ident) -> proc_macro2::TokenStream {
    quote! {
        impl Into<Action> for #name {
            fn into(self) -> Action {
                Action(Box::new(self))
            }
        }
    }
}

// 生成 display 实现
fn generate_display_impl(
    struct_attrs: &StructAttrs,
    field_infos: &[FieldInfo],
    name: &Ident,
) -> proc_macro2::TokenStream {
    // 生成 head 部分
    let head_part = if let Some(head) = &struct_attrs.head {
        quote! { concat!(#head, ":") }
    } else {
        quote! { self.get_head() }
    };

    // 生成 main 部分
    let main_part = if let Some(main_type) = &struct_attrs.main {
        // 找到被标记为 main 的字段（需要完整的 FieldInfo 以便读取 none 标志）
        let main_field = field_infos.iter().find(|info| info.main);

        if let Some(main_field) = main_field {
            let field_ident = &main_field.ident;
            let main_is_option = is_option_type(&main_field.ty);
            let main_none_flag = main_field.none;

            if main_type == "single" {
                if main_is_option {
                    if main_none_flag {
                        // Option + none -> None 时输出 "none"
                        quote! {
                            match &self.#field_ident {
                                Some(v) => format!("{}", v),
                                None => String::from("none"),
                            }
                        }
                    } else {
                        quote! {
                            match &self.#field_ident {
                                Some(v) => format!("{}", v),
                                None => String::new(),
                            }
                        }
                    }
                } else {
                    quote! {
                        format!("{}", self.#field_ident)
                    }
                }
            } else if main_type == "list" {
                if main_is_option {
                    // 对于 list 的 Option：当 Some(arr) 时 join，否则空或 none（如果设置 none，可返回 "none"）
                    if main_none_flag {
                        quote! {
                            {
                                let items: Vec<String> = if let Some(ref arr) = &self.#field_ident {
                                    arr.iter().map(|item| format!("{}", item)).collect()
                                } else {
                                    Vec::new()
                                };
                                if items.is_empty() {
                                    String::from("none")
                                } else {
                                    format!("{}", items.join("|"))
                                }
                            }
                        }
                    } else {
                        quote! {
                            {
                                let items: Vec<String> = if let Some(ref arr) = &self.#field_ident {
                                    arr.iter().map(|item| format!("{}", item)).collect()
                                } else {
                                    Vec::new()
                                };
                                format!("{}", items.join("|"))
                            }
                        }
                    }
                } else {
                    quote! {
                        {
                            let items: Vec<String> = self.#field_ident.iter().map(|item| format!("{}", item)).collect();
                            format!("{}", items.join("|"))
                        }
                    }
                }
            } else {
                panic!("Invalid main type: {main_type}");
            }
        } else {
            panic!(
                "Struct {name} has main = \"{main_type}\" but no field marked with #[action(main)]"
            );
        }
    } else {
        quote! { self.get_main() }
    };

    // 生成 args 部分
    let mut arg_parts = Vec::new();

    for field_info in field_infos {
        if let Some(arg_type) = &field_info.arg {
            let field_ident = &field_info.ident;
            // 如果提供了 rename，则使用 rename 作为参数名称，否则使用字段名
            let field_name = if let Some(r) = &field_info.rename {
                r.clone()
            } else {
                field_ident.to_string()
            };
            let is_option = is_option_type(&field_info.ty);

            let arg_part = if field_info.nullable || is_option {
                // 处理可为空的字段
                match arg_type.as_str() {
                    "tag" => {
                        // 对于 nullable/tag：如果有 Some(true)，先推入 -tie（如果有），再推入 -field_name
                        let tie_name = field_info.tie.clone();
                        // 如果设置了 none 标志，需要为 None 情况输出 -field_name=none 或 -field_name none （对 tag 我们使用 -field_name=none）
                        if field_info.none {
                            match tie_name {
                                Some(tn) => {
                                    quote! {
                                        if let Some(value) = &self.#field_ident {
                                            if *value {
                                                args.push(format!("-{}", #tn));
                                                args.push(format!("-{}", #field_name));
                                            }
                                        } else {
                                            // None 情况输出 -name=none（并且推入 tie）
                                            args.push(format!("-{}", #tn));
                                            args.push(format!("-{}=none", #field_name));
                                        }
                                    }
                                }
                                None => {
                                    quote! {
                                        if let Some(value) = &self.#field_ident {
                                            if *value {
                                                args.push(format!("-{}", #field_name));
                                            }
                                        } else {
                                            args.push(format!("-{}=none", #field_name));
                                        }
                                    }
                                }
                            }
                        } else {
                            match tie_name {
                                Some(tn) => {
                                    quote! {
                                        if let Some(value) = &self.#field_ident {
                                            if *value {
                                                args.push(format!("-{}", #tn));
                                                args.push(format!("-{}", #field_name));
                                            }
                                        }
                                    }
                                }
                                None => {
                                    quote! {
                                        if let Some(value) = &self.#field_ident {
                                            if *value {
                                                args.push(format!("-{}", #field_name));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    "pair" => {
                        // nullable pair: 如果 Some(value)，先推入 -tie（如果有），再推入 -name=value
                        let tie_name = field_info.tie.clone();
                        if field_info.none {
                            match tie_name {
                                Some(tn) => {
                                    quote! {
                                        if let Some(value) = &self.#field_ident {
                                            args.push(format!("-{}", #tn));
                                            args.push(format!("-{}={}", #field_name, format!("{}", value)));
                                        } else {
                                            // None 情况输出 -name=none，同时推入 tie
                                            args.push(format!("-{}", #tn));
                                            args.push(format!("-{}=none", #field_name));
                                        }
                                    }
                                }
                                None => {
                                    quote! {
                                        if let Some(value) = &self.#field_ident {
                                            args.push(format!("-{}={}", #field_name, format!("{}", value)));
                                        } else {
                                            args.push(format!("-{}=none", #field_name));
                                        }
                                    }
                                }
                            }
                        } else {
                            match tie_name {
                                Some(tn) => {
                                    quote! {
                                        if let Some(value) = &self.#field_ident {
                                            args.push(format!("-{}", #tn));
                                            args.push(format!("-{}={}", #field_name, format!("{}", value)));
                                        }
                                    }
                                }
                                None => {
                                    quote! {
                                        if let Some(value) = &self.#field_ident {
                                            args.push(format!("-{}={}", #field_name, format!("{}", value)));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    "value" => {
                        // nullable value: 先输出 -tie（如果有），再输出 -value
                        let tie_name = field_info.tie.clone();
                        if field_info.none {
                            match tie_name {
                                Some(tn) => {
                                    quote! {
                                        if let Some(value) = &self.#field_ident {
                                            args.push(format!("-{}", #tn));
                                            args.push(format!("-{}", format!("{}", value)));
                                        } else {
                                            args.push(format!("-{}", #tn));
                                            args.push(format!("-none"));
                                        }
                                    }
                                }
                                None => {
                                    quote! {
                                        if let Some(value) = &self.#field_ident {
                                            args.push(format!("-{}", format!("{}", value)));
                                        } else {
                                            args.push(format!("-none"));
                                        }
                                    }
                                }
                            }
                        } else {
                            match tie_name {
                                Some(tn) => {
                                    quote! {
                                        if let Some(value) = &self.#field_ident {
                                            args.push(format!("-{}", #tn));
                                            args.push(format!("-{}", format!("{}", value)));
                                        }
                                    }
                                }
                                None => {
                                    quote! {
                                        if let Some(value) = &self.#field_ident {
                                            args.push(format!("-{}", format!("{}", value)));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => panic!("Invalid arg type: {arg_type}"),
                }
            } else {
                // 处理不可为空的字段
                match arg_type.as_str() {
                    "tag" => {
                        // 对于非 nullable 的 tag：如果为 true，先推入 -tie（如果有），再推入 -field_name
                        let tie_name = field_info.tie.clone();
                        match tie_name {
                            Some(tn) => {
                                quote! {
                                    if self.#field_ident {
                                        args.push(format!("-{}", #tn));
                                        args.push(format!("-{}", #field_name));
                                    }
                                }
                            }
                            None => {
                                quote! {
                                    if self.#field_ident {
                                        args.push(format!("-{}", #field_name));
                                    }
                                }
                            }
                        }
                    }
                    "pair" => {
                        // 非 nullable pair: 先推入 -tie（如果有），再推入 -name=value
                        let tie_name = field_info.tie.clone();
                        match tie_name {
                            Some(tn) => {
                                quote! {
                                    args.push(format!("-{}", #tn));
                                    args.push(format!("-{}={}", #field_name, format!("{}", self.#field_ident)));
                                }
                            }
                            None => {
                                quote! {
                                    args.push(format!("-{}={}", #field_name, format!("{}", self.#field_ident)));
                                }
                            }
                        }
                    }
                    "value" => {
                        // 非 nullable value: 先推入 -tie（如果有），再推入 -value
                        let tie_name = field_info.tie.clone();
                        match tie_name {
                            Some(tn) => {
                                quote! {
                                    args.push(format!("-{}", #tn));
                                    args.push(format!("-{}", format!("{}", self.#field_ident)));
                                }
                            }
                            None => {
                                quote! {
                                    args.push(format!("-{}", format!("{}", self.#field_ident)));
                                }
                            }
                        }
                    }
                    _ => panic!("Invalid arg type: {arg_type}"),
                }
            };

            arg_parts.push(arg_part);
        }
    }
    // 生成 Display impl 的最终 token stream。该实现会：
    // 1. 计算 head（优先使用属性指定的静态 head，否则调用 `get_head()`）
    // 2. 计算 main（优先使用属性指定的静态 main，否则调用 `get_main()` 或者通过被标记为 main 的字段生成）
    // 3. 逐个运行之前生成的 arg parts（这些是按字段生成的 snippets），把结果 push 到 args
    // 4. 合并来自 `get_other_args()` 的键值对（如果存在），支持 None 表示纯 flag
    // 5. 最终把 head + main + 可选的 args join 成一个字符串并写入 formatter
    quote! {
        impl std::fmt::Display for #name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let head = #head_part;
                let main = #main_part;

                // 获取 args
                let mut args = Vec::new();
                #(#arg_parts)*

                // 获取 other_args（允许用户在类型中通过方法提供额外参数）
                if let Some(other_args) = self.get_other_args() {
                    for (key, value) in other_args {
                        match value {
                            Some(val) => args.push(format!("-{}={}", key, val)),
                            None => args.push(format!("-{}", key)),
                        }
                    }
                }

                // 组合所有部分
                let s = if args.is_empty() {
                    format!("{}{}", head, main)
                } else {
                    format!("{}{} {}", head, main, args.join(" "))
                };

                write!(f, "{};", s)  // 别忘了行尾分号~
            }
        }
    }
}

// 生成 Actionable 特型 impl
fn generate_actionable_impl(name: &Ident) -> proc_macro2::TokenStream {
    quote! {
        impl Actionable for #name {}
    }
}

// 检查类型是否为 Option
fn is_option_type(ty: &syn::Type) -> bool {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident == "Option";
    }
    false
}
