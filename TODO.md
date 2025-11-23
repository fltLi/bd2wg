# bd2wg v1 重构清单

## 重构目标

- [ ] 重新设计项目架构, 简化内部接口.

- [ ] 提供更稳健和成熟的转译, 下载和打包功能.

- [ ] 实现更美观更友好的命令行界面.

- [ ] 提供友好的 wasm 调用接口封装.

## 具体步骤

1. [ ] 设计并编写 crates/bd2wg 基础类型和接口抽象.

2. [x] 继承 webgal 过程宏, 作为可移植的独立实现 (webgal-derive).

3. [ ] 完成 crates/bd2wg 具体实现.

4. [ ] 完成 crates/bd2wg-cli 命令行界面.

5. [ ] 提供 crates/bd2wg-wasm 封装接口.

6. [ ] 重写编写 README.md, 并在 docs/ 下编写文档. 
