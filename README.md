# Win-way

Windows 端的 Wayland 应用显示器（实验性项目）

## 当前状态

⚠️ **这是一个实验性项目，目前功能非常有限：**

- ✅ 可以创建 GPU 加速的 Windows 窗口
- ✅ 可以接收 TCP 连接
- ✅ 可以显示通过 WPRD 协议发送的帧数据
- ❌ **暂时无法显示 WSL Wayland 应用**（因为无法通过 TCP 传递 Unix 文件描述符）

## 安装

```powershell
git clone https://github.com/J-x-Z/win-way.git
cd win-way
cargo build --release
```

## 使用方法

```powershell
cargo run --release
```

启动后会打开一个窗口，监听 TCP 端口 9999。

## 命令行参数

```
win-way [OPTIONS]
  -p, --port <PORT>    监听端口 (默认: 9999)
  -d, --debug          开启调试日志
```

## 系统要求

- Windows 10+
- 支持 OpenGL 3.3 的显卡
- Rust 1.70+

## 许可证

GPL-3.0
