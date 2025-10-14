# claude-usage-line

> [English](README.md) | [한국어](README-ko.md)

터미널 상태 줄에 Claude CLI 사용량 통계를 실시간으로 표시하는 경량 데몬입니다.

![데모](images/a.png)

## 📋 요구사항

- [Claude Code](https://claude.com/code)
- Rust 툴체인 ([rustup](https://rustup.rs/)으로 설치)

## 🚀 빠른 시작

### 1. 설치

```bash
cargo install --git https://github.com/somersby10ml/claude-usage-line
```

### 2. 커맨드라인 옵션

`claude-usage-line`은 다음 옵션들을 지원합니다:

| 옵션 | 설명 | 기본값 | 예시 | 단위 |
|------|------|--------|------|------|
| `--debug` | 디버그 로깅 활성화 | `false` | `claude-usage-line --debug` | - |
| `--refresh-interval` | 사용량 새로고침 주기 | `4` | `claude-usage-line --refresh-interval 5` | 분 |
| `--idle-timeout` | 데몬 종료 전 대기 시간 | `10` | `claude-usage-line --idle-timeout 15` | 분 |
| `--ccjs` | Claude Code cli.js 커스텀 경로 | (자동 감지) | `claude-usage-line --ccjs /path/to/cli.js` | - |

**참고**: `statusLine.command`에서 사용할 때는 일반적으로 옵션을 지정할 필요가 없습니다. 데몬이 자동으로 기본 설정으로 실행됩니다.

### 3. Claude Code 설정

#### Windows

`%USERPROFILE%\.claude\settings.json`에 추가:

```json
{
  "statusLine": {
    "command": "claude-usage-line"
  }
}
```

![Windows 설정](images/b.png)

#### Linux/macOS

`~/.claude/settings.json`에 추가:

```json
{
  "statusLine": {
    "command": "printf '\\033[01;32m%s@%s\\033[00m:\\033[01;34m%s\\033[00m\\n' \"$(whoami)\" \"$(hostname -s)\" \"$(pwd)\" ; claude-usage-line"
  }
}
```

원하는 대로 커스터마이징해서 사용하세요.

![Linux 설정](images/c.png)

### 4. 캐시 초기화

`claude-usage-line`을 1~2회 수동으로 실행합니다. 처음 1~2회 실행 시 아무것도 안 나오거나 공백으로 나오는 것은 정상입니다.

```bash
claude-usage-line
```

### 5. 캐시 디렉토리 신뢰

캐시 디렉토리에서 `claude`를 실행합니다:

- **Windows**: `%LOCALAPPDATA%\claude-usage-line`
- **Linux/macOS**: `~/.cache/claude-usage-line`

"Do you trust the files in this folder?" 메시지가 나오면 **허용**을 클릭하고 닫습니다.

## 🔧 문제 해결

### 캐시가 업데이트되지 않을 때

첫 설치 및 시작 시 사용량 통계가 표시되지 않거나 업데이트되지 않는 경우:

1. 실행 중인 모든 `claude-usage-line` 프로세스를 종료합니다:
   ```bash
   # Windows
   taskkill /F /IM claude-usage-line.exe

   # Linux/macOS
   pkill claude-usage-line
   ```
2. Claude에게 다시 질문하여 상태 줄을 새로고침합니다

데몬은 기본적으로 4분마다 데이터를 캐시합니다. 변경사항이 즉시 나타나지 않으면 프로세스를 재시작하여 새 캐시를 강제로 생성할 수 있습니다.

## 🗑️ 제거

1. `.claude/settings.json`에서 `claude-usage-line` 구문 제거
2. 실행 중인 모든 프로세스 종료:
   ```bash
   # Windows
   taskkill /F /IM claude-usage-line.exe

   # Linux/macOS
   pkill claude-usage-line
   ```
3. cargo로 제거:
   ```bash
   cargo uninstall claude-usage-line
   ```

## 💬 이슈

버그를 발견하거나 제안사항이 있나요? [이슈](../../issues)에 자유롭게 남겨주세요!
