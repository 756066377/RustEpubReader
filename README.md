# RustEpubReader

面向 Windows x86_64 桌面环境的轻量 EPUB 阅读器，重点优化连续阅读、快速隐藏和低干扰使用体验。

## 下载

从 [GitHub Releases](https://github.com/756066377/RustEpubReader/releases/latest) 下载最新正式版：

- Windows 64 位：`RustEpubReader-Win64-v*.exe`
- 完整版本变化见 [CHANGELOG.md](CHANGELOG.md)

[GitHub Actions](https://github.com/756066377/RustEpubReader/actions) 中的 `windows-desktop-build` 是每次推送产生的开发构建，不保证等同于正式版。

## 主要功能

- EPUB 打开、书库管理和最近阅读记录
- 按章节按需加载，支持连续跨章节滚动
- 目录跳转、章节书签和块级阅读位置恢复
- 全文搜索、标注、笔记和阅读统计
- TTS 连续朗读
- 背景色、背景图、字体、字号、行距和标题倍率调整
- 跨设备阅读进度同步
- 命令行打开 `.epub` 和 `.txt`
- 中文与 English 界面

## 隐蔽阅读

- 无标题栏窗口，阅读区域保持简洁
- 阅读窗口可保持置顶
- 空白区域可拖动，窗口边缘可调整大小
- 默认 `F1` 全局隐藏或显示窗口，也可在设置中修改
- `F2` 显示或隐藏选项栏；隐藏时会一并关闭目录、设置等面板
- 阅读模式隐藏正文滚动条，支持滚轮和触控板连续滚动
- 背景透明度只作用于阅读背景，文字保持清晰

## 快捷键

| 快捷键 | 功能 |
| --- | --- |
| `F1` | 全局隐藏或显示阅读器，支持在设置中修改 |
| `F2` | 显示或隐藏阅读选项栏及其附属面板 |
| 滚轮 / 触控板 | 连续滚动阅读 |

## 构建

环境要求：

- Rust stable
- Windows MSVC C++ Build Tools

```bash
cargo build --release -p rust_epub_reader
```

生成文件位于 `target/release/rust_epub_reader.exe`。

推送任意分支会生成 GitHub Actions 开发构建。推送 `v*` 标签时，workflow 会完成以下步骤：

1. 在 Windows runner 上编译发行二进制。
2. 从 [CHANGELOG.md](CHANGELOG.md) 提取与标签同名的版本说明。
3. 创建 GitHub Release 并上传 Windows 可执行文件。

如果标签在更新日志中没有对应章节，正式发布任务会失败，避免发布无说明的版本。

## 项目结构

```text
RustEpubReader/
|-- core/           # EPUB 解析、书库、搜索、同步等核心逻辑
|-- desktop/        # Windows 桌面端（egui / eframe）
|-- android/        # Android 工程
`-- android-bridge/ # Android JNI 桥接
```

## 定位

这个分支主要面向 Windows 办公环境，优先考虑低暴露、快速隐藏、连续阅读和较低的前台资源占用，而不是追求所有平台完全一致的体验。

## License

见 [LICENSE](LICENSE)。
