# bd2wg 项目设计

阅读此文档有助于理解 bd2wg 项目的组织和运行方式, 以便更好地使用, 修改和添加代码.

## 架构

bd2wg 由 **100% Rust** 开发, 包含 `bd2wg`, `bd2wg-cli`, `webgal-derive`, `webgal-derive-macro` 四个 crate.

### crates/bd2wg

核心业务逻辑, 主要包含以下模块:

- `models`: 提供脚本, Live2D 模型等的数据结构.

- `traits`: 功能相关特型, 方便后期扩展实现.

  - `Resolve`: 解析 Bestdori 资源, 获取 url 及写入路径.

  - `Transpile`: 转译脚本.

  - `Download`: 下载相关资源 (包括 Live2D 资源的进一步解析).

  - `Pipeline`: 上述抽象组合成的工作管线, 分为 `TranspilePipeline` 和 `DownloadPipeline`.

- `services`: 上述抽象的具体实现.

> [!NOTE]
> 
> `bd2wg` 与 `bd2wg-cli` 基本没有耦合, 相关接口也并非为其而设计, 因此具备移植到其他形式应用的条件.

### crates/bd2wg-cli

简单的流程控制和终端交互实现.

### crates/webgal-derive, crates/webgal-derive-macro

高自由度的 WebGAL 脚本指令序列化派生宏, 支持在其他项目中复用.

## 贡献

若您有 issue, pr, 仓库作者可能只会在周日回复 (但一定会回复), 请谅解!
