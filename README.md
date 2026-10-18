# winch

> 开发中。配置格式、命令行参数和行为都可能调整,暂不建议用于正式项目。
>
> 这是一个个人向的小工具,主要用来解决自己在 Windows 上管理开发环境变量的麻烦,按需迭代。

`winch` 是一个 Windows 上的声明式环境变量管理工具。它读取当前目录的 `winch.toml`,解析 MSVC、vcpkg、JDK、MSYS2 等工具链,把环境变量注入到子进程,或者导出成 shell 脚本。

## 能做什么

- 按项目指定 MSVC 版本和平台,自动调用 `vcvarsall.bat` 组合环境
- 可选启用 vcpkg,指定 triplet
- 可选注入 MSYS2 环境,选择 MSYS / MinGW 类型
- 可选按版本要求选择 JDK
- 通过 `[env]` 覆盖自定义环境变量,或用 `[env.prepend]` / `[env.append]` 在值首尾追加
- 用 `pure = true` 只保留解析出的环境变量,不继承调用者的环境
- 用 `winch shell` / `winch run` 在准备好的环境中启动 shell 或程序
- 用 `winch env` 打印加载后的完整环境变量
- 用 `winch diff` 查看环境变化及每条变化的来源
- 把解析结果缓存到 `.winch/cache.toml`,减少重复解析
- 导出 bash、PowerShell、Nushell 可用的环境脚本

## Todo List

- [x] 环境探测
- [x] 启动shell并注入环境
- [x] list搜索环境
- [x] check检查
- [x] MSYS2 环境注入支持
- [x] `diff`命令生成诊断信息
- [ ] 支持管理配置更多的平台（例如：Ninja、Conan、Python、Nodejs等）
- [ ] `preset`命令生成 **CMake Presets**
- [ ] `preset`命令生成 **compile_commands.json**
- [ ] `preset`命令生成 **VSCode Tasks**
- [ ] `preset`命令生成 **VSCode Launch**
- [ ] `init`命令交互式创建`winch.toml`
- [ ] 更好的缓存失效机制
- [ ] 跨平台 (最低优先级)

## 环境要求

- Windows
- 支持 edition 2024 的 Rust stable
- 使用 MSVC 功能时需要 `vswhere.exe`(通常随 Visual Studio Installer 安装;也可以通过 winget 或 scoop 安装)
- vcpkg、JDK、MSYS2 按需准备

## 构建

```powershell
cargo build --release
```

生成的程序位于 `target/release/winch.exe`。

CI 会在 `windows-latest` 上运行格式检查、clippy、测试和 release 构建。推送 `v*` 标签(例如 `v0.1.0`)会自动创建 GitHub Release,并上传包含 `winch.exe`、`README.md` 和 `LICENSE` 的 `winch-<tag>-windows-x64.zip`。

## 使用

### 项目配置

在项目根目录创建 `winch.toml`:

```toml
[toolchain]
java = "17"

[toolchain.msvc]
version = "17"
platform = "x64"

[toolchain.msys2]
enable = true
type = "ucrt64"

[dev.vcpkg]
enable = true
triplet = "x64-windows"

[env]
FOO = "bar"
```

字段说明:

| 字段                      | 说明                                                                                  |
|---------------------------|---------------------------------------------------------------------------------------|
| `pure`                    | 是否清空继承的环境变量,只保留解析结果,默认 `false`                                    |
| `toolchain.msvc.version`  | Visual Studio 版本,最多 4 段。省略的段按通配处理,例如 `17` 匹配任意 `17.x`            |
| `toolchain.msvc.platform` | 传给 `vcvarsall.bat` 的平台,如 `x64`、`x86`、`arm64`                                  |
| `toolchain.msys2.enable`  | 是否启用 MSYS2 环境,默认 `false`                                                      |
| `toolchain.msys2.type`    | MSYS2 环境类型:`ucrt64`(默认)、`mingw64`、`mingw32`、`clang64`、`clangarm64`、`msys2` |
| `toolchain.java`          | JDK 的 semver 版本要求,如 `17`、`>=17, <21`                                           |
| `dev.vcpkg.enable`        | 是否启用 vcpkg,默认 `false`                                                           |
| `dev.vcpkg.triplet`       | vcpkg triplet,如 `x64-windows`                                                        |
| `env`                     | 自定义环境变量,键和值都必须是字符串;`env.prepend` / `env.append` 用于在现有值首尾追加 |

所有段都是可选的,只写需要的部分即可;但一旦写了某个工具链,它内部的字段需要完整。

### MSYS2

启用后会设置 `MSYSTEM`,按类型设置 `MINGW_PACKAGE_PREFIX`,并把实际存在的 `usr/local/bin`、`usr/bin` 和对应环境的 `bin` 加入 `Path`:

```toml
[toolchain.msys2]
enable = true
type = "ucrt64"
```

### 自定义环境变量

`[env]` 在工具链解析之后应用,同名变量会覆盖工具链生成的值。需要保留现有值并在开头或结尾追加时,使用 `[env.prepend]` 或 `[env.append]`:

```toml
[env]
FOO = "bar"

[env.prepend]
Path = "C:\\tools\\bin"

[env.append]
Path = "C:\\other\\bin"
```

几点注意:

- 值是字面量,不支持 `%VAR%` 或 `${env:VAR}` 之类的变量展开
- 值需要是 TOML 字符串,`PORT = 8080` 这种写法会解析失败,应写成 `PORT = "8080"`
- `[env]` 会 **整体替换**变量,包括 `Path`、`LIB`、`INCLUDE`;`[env.prepend]` / `[env.append]` 则用 `;` 拼接在现有值的开头或结尾
- 同一个变量可以同时出现在 `[env.prepend]` 和 `[env.append]`,会先应用 prepend 再应用 append
- 同一个变量不要同时写在 `[env]` 和修改段中;修改段会覆盖 `[env]` 的声明
- 变量名按 Windows 规则不区分大小写;例如写 `PATH` 会合并到继承环境里的 `Path`,不会产生两个变量

### 纯净模式

默认情况下,`winch env` / `winch shell` / `winch run` 会在当前进程环境的基础上叠加解析结果。在 `winch.toml` 顶层设置 `pure = true` 后,这些命令只保留解析出的变量,不再继承调用者的环境:

```toml
pure = true

[env]
SystemRoot = "C:\\Windows"
TEMP = "C:\\Windows\\Temp"
```

几点注意:

- 开启后 `SystemRoot`、`TEMP`、`USERPROFILE`、`Path` 等常用变量都会被清除;如果启动的 shell 或程序依赖它们,需要在 `[env]` 中显式补回
- `pure` 只影响 `env`、`shell`、`run`;`export` 生成的脚本仍然只包含声明/解析出的变量,不会清空目标 shell 的环境
- `check` 只校验解析结果,不受 `pure` 影响

### 命令

```powershell
# 校验配置中的工具链,并检查解析出的 Path 目录
winch check

# 列出 PATH 中检测到的 clang / gcc / java,以及 MSYS2 和 Visual Studio
winch list

# 打印加载后的完整环境变量(按变量名排序)
winch env

# 查看应用 winch.toml 后的环境变化及来源
winch diff

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

全局参数 (写在子命令之前):

```powershell
winch --verbose check
winch --force shell
winch --no-log env
```

- `-v, --verbose`:输出调试日志
- `-f, --force`:忽略缓存,重新解析环境
- `--no-log`:不输出日志(检查失败时仍会打印错误摘要)

`winch env` 默认输出进程继承的完整环境加上 `winch.toml` 的结果,按变量名排序;开启 `pure` 后只输出解析结果。需要干净输出时配合 `--no-log` 使用。

### 差异

`winch diff` 比较当前进程环境和应用 `winch.toml` 后的环境,按变量名列出变化,并在每条追加/替换后标注来源:

```text
Path
  + C:\msvc\bin  (msvc)
  + C:\vcpkg\bin  (vcpkg)

FOO
  - old  (inherited)
  + bar  (user config)
```

- 新增或 prepend/append 的值显示为 `+`;`[env]` 整体替换会同时显示 `-` 旧值和 `+` 新值
- 来源包括 `msvc`、`vcpkg`、`java`、`msys2` 和 `user config`
- 开启 `pure` 时,被清除的继承变量只汇总数量;使用 `--verbose` 可以在日志中查看具体变量名
- 和 `env` / `shell` / `run` 一样使用缓存,需要重新解析时加 `--force`

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
- 修改 `winch.toml`(包括 `[env]`)会自动重建缓存;工具链升级或移动后请使用 `--force`
- 缓存格式带版本号,格式不匹配时会自动重建

### 导出

`winch export` 会把当前解析结果写成脚本,默认输出到:

- `.winch/env.sh`
- `.winch/env.ps1`
- `.winch/env.nu`

bash 导出会把 Windows 路径转换成 `/c/...` 形式,适合 Git Bash / MSYS2 环境;PowerShell 和 Nushell 使用原生路径。

## 目前尚未完成

- 仅支持 Windows
- clang、gcc 目前只在 `list` 中检测,还没有注入环境
- `check` 的检查项还比较基础:工具链是否可用、Path 目录是否存在
- `[env]` 不支持变量展开;追加请使用 `[env.prepend]` / `[env.append]`
- 缓存只根据 `winch.toml` 判断是否有效,不会自动感知工具链变化
- 测试覆盖有限,接口也在调整中

## 代码结构

```
src/
  main.rs        入口,日志与退出码
  cli.rs         命令行参数
  app.rs         命令调度与环境加载
  env.rs         环境变量增量模型(set / prepend / append,含来源信息)
  cache.rs       缓存读写
  check.rs       环境检查(开发中)
  diff.rs        环境差异与来源展示
  export.rs      脚本导出
  find.rs        在 PATH 中查找工具
  version.rs     clang / gcc / JDK 版本解析
  msvc.rs        vswhere、VS 版本、vcvarsall 环境提取
  msys2.rs       MSYS2 检测
  winapi.rs      Windows API 辅助
  toolchain/     MSVC、vcpkg、JDK、MSYS2 的环境解析
```
