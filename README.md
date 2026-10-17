# winch

> 开发中。配置格式、命令行参数和行为都可能调整,暂不建议用于正式项目。
>
> 这是一个个人向的小工具,主要用来解决自己在 Windows 上管理开发环境变量的麻烦,按需迭代。

`winch` 是一个 Windows 上的声明式环境变量管理工具。它读取当前目录的 `winch.toml`,解析 MSVC、vcpkg、JDK 等工具链,把环境变量注入到子进程,或者导出成 shell 脚本。

## 能做什么

- 按项目指定 MSVC 版本和平台,自动调用 `vcvarsall.bat` 组合环境
- 可选启用 vcpkg,指定 triplet
- 可选按版本要求选择 JDK
- 用 `winch shell` / `winch run` 在准备好的环境中启动 shell 或程序
- 把解析结果缓存到 `.winch/cache.toml`,减少重复解析
- 导出 bash、PowerShell、Nushell 可用的环境脚本

## 环境要求

- Windows
- 支持 edition 2024 的 Rust stable
- 使用 MSVC 功能时需要 `vswhere.exe`(通常随 Visual Studio Installer 安装;也可以通过 winget 或 scoop 安装)
- vcpkg、JDK 按需准备

## 构建

```powershell
cargo build --release
```

生成的程序位于 `target/release/winch.exe`。

## 使用

### 项目配置

在项目根目录创建 `winch.toml`:

```toml
[toolchain]
java = "17"

[toolchain.msvc]
version = "17"
platform = "x64"

[dev.vcpkg]
enable = true
triplet = "x64-windows"
```

字段说明:

| 字段 | 说明 |
| --- | --- |
| `toolchain.msvc.version` | Visual Studio 版本,最多 4 段。省略的段按通配处理,例如 `17` 匹配任意 `17.x` |
| `toolchain.msvc.platform` | 传给 `vcvarsall.bat` 的平台,如 `x64`、`x86`、`arm64` |
| `toolchain.java` | JDK 的 semver 版本要求,如 `17`、`>=17, <21` |
| `dev.vcpkg.enable` | 是否启用 vcpkg,默认 `false` |
| `dev.vcpkg.triplet` | vcpkg triplet,如 `x64-windows` |

所有段都是可选的,只写需要的部分即可;但一旦写了某个工具链,它内部的字段需要完整。

### 命令

```powershell
# 解析配置,做基础检查
winch check

# 列出 PATH 中检测到的 clang / gcc / java,以及 MSYS2 和 Visual Studio
winch list

# 在准备好的环境中启动 shell
winch shell

# 在准备好的环境中运行程序
winch run cargo build

# 导出环境脚本
winch export --shell powershell
winch export --shell bash
winch export --shell nu

# 输出到 stdout
winch export --shell bash --output -
```

全局参数(写在子命令之前):

```powershell
winch --verbose check
winch --force shell
```

- `-v, --verbose`:输出调试日志
- `-f, --force`:忽略缓存,重新解析环境

### 用户配置

可在用户目录下创建 `~/.winch.toml`,配置 `winch shell` 启动的 shell:

```toml
[shell]
program = "powershell.exe"
args = ["-NoLogo"]
```

不配置时默认启动 `powershell.exe`。

### 缓存

- 缓存位于 `.winch/cache.toml`,以 `winch.toml` 的 SHA-256 作为键
- 缓存里是解析后的绝对路径,属于本机数据,建议把 `/.winch/` 加入项目自己的 `.gitignore`
- 修改 `winch.toml` 会自动重建缓存;工具链升级或移动后请使用 `--force`
- 缓存格式带版本号,格式不匹配时会自动重建

### 导出

`winch export` 会把当前解析结果写成脚本,默认输出到:

- `.winch/env.sh`
- `.winch/env.ps1`
- `.winch/env.nu`

bash 导出会把 Windows 路径转换成 `/c/...` 形式,适合 Git Bash / MSYS2 环境;PowerShell 和 Nushell 使用原生路径。

## 目前尚未完成

- 仅支持 Windows
- clang、gcc、MSYS2 目前只在 `list` 中检测,还没有注入环境
- `check` 目前只做基础的路径存在性检查,其他检查还在补充
- 缓存只根据 `winch.toml` 判断是否有效,不会自动感知工具链变化
- 测试覆盖有限,接口也在调整中

## 代码结构

```
src/
  main.rs        入口,日志与退出码
  cli.rs         命令行参数
  app.rs         命令调度与环境加载
  env.rs         环境变量增量模型(set / prepend)
  cache.rs       缓存读写
  check.rs       环境检查(开发中)
  export.rs      脚本导出
  find.rs        在 PATH 中查找工具
  version.rs     clang / gcc / JDK 版本解析
  msvc.rs        vswhere、VS 版本、vcvarsall 环境提取
  msys2.rs       MSYS2 检测
  winapi.rs      Windows API 辅助
  toolchain/     MSVC、vcpkg、JDK 的环境解析
```
