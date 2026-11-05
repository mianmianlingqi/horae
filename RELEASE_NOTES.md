# v0.2.6-mouse.1

基于 zhaohang1205/horae v0.2.5 的鼠标交互增强版本，保留 GPL-3.0 许可证、中文界面、键盘快捷键与原有数据格式。

## 功能

- 点击任务行选中任务并更新详情，点击行首状态标记完成任务或恢复已完成任务。
- 点击侧栏切换视图，点击面板切换焦点。
- 导航、任务列表、详情、帮助、语法说明和工作流独立滚动。
- 弹窗选项、确认、取消、输入提交及补全候选支持点击，模态层屏蔽底层事件。
- F7 可设置鼠标开关和滚轮步长，兼容旧设置。
- 增加终端状态恢复、中文和多行任务坐标映射及 26 项鼠标回归测试。

## Windows 使用

下载 `horae-v0.2.6-mouse.1-x86_64-pc-windows-msvc.zip`，解压后在 Windows Terminal 中运行 `horae.exe`，不需要安装 Rust。现有任务和配置继续存放在 `%APPDATA%\horae`，发布包不包含任何用户数据。SHA256 文件可用于校验下载内容。

对应源码：[mianmianlingqi/horae v0.2.6-mouse.1](https://github.com/mianmianlingqi/horae/tree/v0.2.6-mouse.1)。上游项目：[zhaohang1205/horae](https://github.com/zhaohang1205/horae)。

## 验证及限制

发布准备已通过格式检查、workspace 编译与 Clippy 严格检查。Windows 环境下，排除 6 项已记录的平台相关失败或未结束测试后，391 项测试通过，其中包含 26 项鼠标测试。5 项失败已在未修改的上游源码中复现，另有 1 项番茄钟守护进程集成测试在本机长时间未结束。

排除项：`db_path_keeps_profile_file` 两项、`db_path_relative_resolves_under_config_dir`、`reminder_written_once_and_catch_up`、`test_voice_capture_cancel_and_flow`、`pomo_start_accepts_id_prefix`。未过滤的 Windows 完整测试并非全部通过。

文字编辑和全屏番茄专注操作保留原有键盘入口。真实终端中的鼠标报告、原生文本选择及终端恢复仍建议按 README 的步骤验证。
