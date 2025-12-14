extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Attribute, Data, DeriveInput, Fields, Ident, Lit, Meta, NestedMeta, Type, parse_macro_input,
};

/// 为具名结构体派生 Actionable trait
///
/// 生成:
/// - `Display`: 格式化为 WebGAL 命令字符串 (head + main + args)
/// - `Into<Action>`: 装箱为通用 Action
/// - `Actionable`: 标记实现
/// - `ActionCustom`: 空实现 (除非标注 #[action(custom)])
///
/// 结构体属性:
/// - `#[action(head = "...")]`: 静态 head 前缀
/// - `#[action(main = "single"|"list")]`: main 序列化方式
/// - `#[action(custom)]`: 用户自定义 ActionCustom
///
/// 字段属性:
/// - `#[action(main)]`: 标记 main 字段
/// - `#[action(nullable)]`: 字段可为空 (通常 Option<T>)
/// - `#[action(none)]`: None 时输出 "none"
/// - `#[action(arg = "tag"|"pair"|"value")]`: 参数格式
/// - `#[action(rename = "...")]`: 参数重命名
/// - `#[action(tie = "...")]`: 关联开关
#[proc_macro_derive(Actionable, attributes(action))]
pub fn derive_actionable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let struct_attrs = parse_struct_attrs(&input.attrs);

    let fields = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => fields.named,
            _ => panic!("Only named-field structs are supported"),
        },
        _ => panic!("Only structs are supported"),
    };

    let field_infos: Vec<_> = fields.into_iter().map(parse_field_attrs).collect();

    let custom_impl = if struct_attrs.custom {
        quote! {}
    } else {
        gen_action_custom_impl(&name)
    };

    let actionable_impl = gen_actionable_impl(&name);
    let into_action_impl = gen_into_action_impl(&name);
    let display_impl = gen_display_impl(&struct_attrs, &field_infos, &name);

    TokenStream::from(quote! {
        #custom_impl
        #display_impl
        #actionable_impl
        #into_action_impl
    })
}

struct StructAttrs {
    head: Option<String>,
    main: Option<String>,
    custom: bool,
}

fn parse_struct_attrs(attrs: &[Attribute]) -> StructAttrs {
    let mut head = None;
    let mut main = None;
    let mut custom = false;

    for attr in attrs {
        if !attr.path.is_ident("action") {
            continue;
        }

        let Ok(Meta::List(meta_list)) = attr.parse_meta() else {
            continue;
        };

        for nested in meta_list.nested {
            let NestedMeta::Meta(meta) = nested else {
                continue;
            };

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
                Meta::Path(path) if path.is_ident("custom") => {
                    custom = true;
                }
                _ => {}
            }
        }
    }

    StructAttrs { head, main, custom }
}

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

fn parse_field_attrs(field: syn::Field) -> FieldInfo {
    let ident = field.ident.expect("Field must have an identifier");
    let ty = field.ty;
    let mut main = false;
    let mut arg = None;
    let mut rename = None;
    let mut tie = None;
    let mut none = false;
    let mut nullable = false;

    for attr in field.attrs {
        if !attr.path.is_ident("action") {
            continue;
        }

        let Ok(Meta::List(meta_list)) = attr.parse_meta() else {
            continue;
        };

        for nested in meta_list.nested {
            let NestedMeta::Meta(meta) = nested else {
                continue;
            };

            match meta {
                Meta::Path(path) => {
                    if path.is_ident("main") {
                        main = true;
                    } else if path.is_ident("nullable") {
                        nullable = true;
                    } else if path.is_ident("none") {
                        none = true;
                    }
                }
                Meta::NameValue(nv) => {
                    if nv.path.is_ident("arg")
                        && let Lit::Str(lit) = nv.lit
                    {
                        arg = Some(lit.value());
                    } else if nv.path.is_ident("rename")
                        && let Lit::Str(lit) = nv.lit
                    {
                        rename = Some(lit.value());
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

    if none && arg.as_deref() == Some("tag") {
        panic!("#[action(none)] cannot be used with #[action(arg = \"tag\")]");
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

fn gen_action_custom_impl(name: &Ident) -> proc_macro2::TokenStream {
    quote! {
        impl webgal_derive::ActionCustom for #name {}
    }
}

fn gen_into_action_impl(name: &Ident) -> proc_macro2::TokenStream {
    quote! {
        impl Into<Action> for #name {
            fn into(self) -> Action {
                Action(Box::new(self))
            }
        }
    }
}

fn gen_actionable_impl(name: &Ident) -> proc_macro2::TokenStream {
    quote! {
        impl webgal_derive::Actionable for #name {}
    }
}

fn gen_display_impl(
    struct_attrs: &StructAttrs,
    field_infos: &[FieldInfo],
    name: &Ident,
) -> proc_macro2::TokenStream {
    let head_part = if let Some(head) = &struct_attrs.head {
        quote! { concat!(#head, ":") }
    } else {
        quote! { self.get_head() }
    };

    let main_part = gen_main_part(struct_attrs, field_infos, name);
    let arg_parts = gen_arg_parts(field_infos);

    quote! {
        impl std::fmt::Display for #name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let head = #head_part;
                let main = #main_part;

                let mut args = Vec::new();
                #(#arg_parts)*

                if let Some(other_args) = self.get_other_args() {
                    for (key, value) in other_args {
                        match value {
                            Some(val) => args.push(format!("-{}={}", key, val)),
                            None => args.push(format!("-{}", key)),
                        }
                    }
                }

                let s = if args.is_empty() {
                    format!("{}{}", head, main)
                } else {
                    format!("{}{} {}", head, main, args.join(" "))
                };

                write!(f, "{};", s)
            }
        }
    }
}

fn gen_main_part(
    struct_attrs: &StructAttrs,
    field_infos: &[FieldInfo],
    name: &Ident,
) -> proc_macro2::TokenStream {
    let Some(main_type) = &struct_attrs.main else {
        return quote! { self.get_main() };
    };

    let main_field = field_infos.iter().find(|info| info.main);
    let Some(main_field) = main_field else {
        panic!(
            "Struct {name} sets main = \"{main_type}\" but has no field marked with #[action(main)]"
        );
    };

    let field_ident = &main_field.ident;
    let is_option = is_option_type(&main_field.ty);
    let none_flag = main_field.none;

    match main_type.as_str() {
        "single" => {
            if is_option {
                if none_flag {
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
                quote! { format!("{}", self.#field_ident) }
            }
        }
        "list" => {
            if is_option {
                if none_flag {
                    quote! {
                        {
                            let items: Vec<String> = self.#field_ident
                                .as_ref()
                                .map(|arr| arr.iter().map(|item| format!("{}", item)).collect())
                                .unwrap_or_default();
                            if items.is_empty() {
                                String::from("none")
                            } else {
                                items.join("|")
                            }
                        }
                    }
                } else {
                    quote! {
                        {
                            let items: Vec<String> = self.#field_ident
                                .as_ref()
                                .map(|arr| arr.iter().map(|item| format!("{}", item)).collect())
                                .unwrap_or_default();
                            items.join("|")
                        }
                    }
                }
            } else {
                quote! {
                    {
                        let items: Vec<String> = self.#field_ident
                            .iter()
                            .map(|item| format!("{}", item))
                            .collect();
                        items.join("|")
                    }
                }
            }
        }
        _ => panic!("Invalid main type: {main_type}"),
    }
}

fn gen_arg_parts(field_infos: &[FieldInfo]) -> Vec<proc_macro2::TokenStream> {
    let mut parts = Vec::new();

    for info in field_infos {
        let Some(arg_type) = &info.arg else {
            continue;
        };

        let field_ident = &info.ident;
        let field_ident_string = field_ident.to_string();
        let field_name = info.rename.as_deref().unwrap_or(&field_ident_string);
        let is_option = is_option_type(&info.ty);

        let part = if info.nullable || is_option {
            gen_nullable_arg(arg_type, info, field_ident, field_name)
        } else {
            gen_non_nullable_arg(arg_type, info, field_ident, field_name)
        };

        parts.push(part);
    }

    parts
}

fn gen_nullable_arg(
    arg_type: &str,
    info: &FieldInfo,
    field_ident: &Ident,
    field_name: &str,
) -> proc_macro2::TokenStream {
    let tie_name = &info.tie;
    let none_flag = info.none;

    match arg_type {
        "tag" => {
            if none_flag {
                match tie_name {
                    Some(tn) => quote! {
                        if let Some(value) = &self.#field_ident {
                            if *value {
                                args.push(format!("-{}", #tn));
                                args.push(format!("-{}", #field_name));
                            }
                        } else {
                            args.push(format!("-{}", #tn));
                            args.push(format!("-{}=none", #field_name));
                        }
                    },
                    None => quote! {
                        if let Some(value) = &self.#field_ident {
                            if *value {
                                args.push(format!("-{}", #field_name));
                            }
                        } else {
                            args.push(format!("-{}=none", #field_name));
                        }
                    },
                }
            } else {
                match tie_name {
                    Some(tn) => quote! {
                        if let Some(value) = &self.#field_ident {
                            if *value {
                                args.push(format!("-{}", #tn));
                                args.push(format!("-{}", #field_name));
                            }
                        }
                    },
                    None => quote! {
                        if let Some(value) = &self.#field_ident {
                            if *value {
                                args.push(format!("-{}", #field_name));
                            }
                        }
                    },
                }
            }
        }
        "pair" => {
            if none_flag {
                match tie_name {
                    Some(tn) => quote! {
                        if let Some(value) = &self.#field_ident {
                            args.push(format!("-{}", #tn));
                            args.push(format!("-{}={}", #field_name, format!("{}", value)));
                        } else {
                            args.push(format!("-{}", #tn));
                            args.push(format!("-{}=none", #field_name));
                        }
                    },
                    None => quote! {
                        if let Some(value) = &self.#field_ident {
                            args.push(format!("-{}={}", #field_name, format!("{}", value)));
                        } else {
                            args.push(format!("-{}=none", #field_name));
                        }
                    },
                }
            } else {
                match tie_name {
                    Some(tn) => quote! {
                        if let Some(value) = &self.#field_ident {
                            args.push(format!("-{}", #tn));
                            args.push(format!("-{}={}", #field_name, format!("{}", value)));
                        }
                    },
                    None => quote! {
                        if let Some(value) = &self.#field_ident {
                            args.push(format!("-{}={}", #field_name, format!("{}", value)));
                        }
                    },
                }
            }
        }
        "value" => {
            if none_flag {
                match tie_name {
                    Some(tn) => quote! {
                        if let Some(value) = &self.#field_ident {
                            args.push(format!("-{}", #tn));
                            args.push(format!("-{}", format!("{}", value)));
                        } else {
                            args.push(format!("-{}", #tn));
                            args.push(format!("-none"));
                        }
                    },
                    None => quote! {
                        if let Some(value) = &self.#field_ident {
                            args.push(format!("-{}", format!("{}", value)));
                        } else {
                            args.push(format!("-none"));
                        }
                    },
                }
            } else {
                match tie_name {
                    Some(tn) => quote! {
                        if let Some(value) = &self.#field_ident {
                            args.push(format!("-{}", #tn));
                            args.push(format!("-{}", format!("{}", value)));
                        }
                    },
                    None => quote! {
                        if let Some(value) = &self.#field_ident {
                            args.push(format!("-{}", format!("{}", value)));
                        }
                    },
                }
            }
        }
        _ => panic!("Invalid arg type: {arg_type}"),
    }
}

fn gen_non_nullable_arg(
    arg_type: &str,
    info: &FieldInfo,
    field_ident: &Ident,
    field_name: &str,
) -> proc_macro2::TokenStream {
    let tie_name = &info.tie;

    match arg_type {
        "tag" => match tie_name {
            Some(tn) => quote! {
                if self.#field_ident {
                    args.push(format!("-{}", #tn));
                    args.push(format!("-{}", #field_name));
                }
            },
            None => quote! {
                if self.#field_ident {
                    args.push(format!("-{}", #field_name));
                }
            },
        },
        "pair" => match tie_name {
            Some(tn) => quote! {
                args.push(format!("-{}", #tn));
                args.push(format!("-{}={}", #field_name, format!("{}", self.#field_ident)));
            },
            None => quote! {
                args.push(format!("-{}={}", #field_name, format!("{}", self.#field_ident)));
            },
        },
        "value" => match tie_name {
            Some(tn) => quote! {
                args.push(format!("-{}", #tn));
                args.push(format!("-{}", format!("{}", self.#field_ident)));
            },
            None => quote! {
                args.push(format!("-{}", format!("{}", self.#field_ident)));
            },
        },
        _ => panic!("无效的 arg 类型: {arg_type}"),
    }
}

fn is_option_type(ty: &syn::Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };

    type_path
        .path
        .segments
        .last()
        .map(|segment| segment.ident == "Option")
        .unwrap_or(false)
}
