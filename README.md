# bd2wg

Bestdori $\rarr$ WebGAL 转换器.

bd2wg 能将 Bestdori 脚本转译为 WebGAL 脚本, 并**自动下载相关资源**, 构建 WebGAL 项目.

<br>

## 特性

---

- **便捷使用**:  无需配置, 开箱即用. 只需输入 Bestdori 脚本和 WebGAL 项目位置, 程序将自动执行转译流程.

- **高效转换**:  流水线设计, 异步资源下载.  秒钟级转换耗时.

- **兼容性**:  对脚本无特殊限制. 自动生成 WebGAL Live2D 配置文件.

<br>

## 安装与使用

---

#### 安装

访问项目 Release 页面, 下载压缩包并解压.

压缩包包含 `bd2wg-cli.exe`, `assets` 两个必要项.

`assets` 中, `bestdori.json` 存储了一些链接生成的辅助信息, `header.json` 则是下载器使用的请求头.

#### 使用

运行 `bd2wg-cli.exe`, 按照提示输入 Bestdori 脚本位置及 WebGAL 项目位置.

> [!NOTE]
>
> 如果您是从 故事$\rarr$导出 获取的脚本, 直接保存即可.
>
> 如果您是通过爬取已发布的故事获取的脚本, 请仅保留 `storySource` 下的内容.

> [!WARNING]
>
> 程序只会将场景文件, 音频, 图片, Live2D 模型输出到 WebGAL 项目中.
>
> 若存在重名内容, 将不询问直接覆盖 (一般不可能重名).

#### 显示信息

bd2wg-cli 转译过程中, 将会显示进度及错误信息.

如果不是致命错误, bd2wg 将不会停止转译过程. (您可以按 ctrl+c 强制中断)

> [!TIP]
>
> 频繁的下载请求可能导致超时等问题, 您可能会在错误输出中看见.
>
> 如果不嫌麻烦, 您可以将错误信息中的 url 复制到浏览器, 再下载到 path 对应位置.

<br>

## 项目设计

---

bd2wg 使用 **100% Rust** 编写, 分为三个模块: 

- `bd2wg`:  业务逻辑库.

  下属 `models` 包含 Bestdori, WebGAL 等数据结构, `pipeline` 为工作流程定义和实现.

  `pipeline` 分为 反序列化$\rarr$预处理+资源解析$\rarr$下载+转译$\rarr$打包 共 6 个抽象, 默认实现均可替换.

- `bd2wg-cli`:  简单的终端交互.

- `proc-macro/action`:  WebGAL 脚本序列化派生宏.

如果您是开发者, 您可以随时 pull 本项目, 提交修改或**使用 bd2wg 源码的任何部分**.

#### 待解决的问题

\* 如果你发现了 bug, 请发 issue.

- (可解决) 适配小概率出现的非 general 模型表情和动作.
