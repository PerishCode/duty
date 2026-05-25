#!/usr/bin/env sh
set -eu

COMMAND=${1:-install}
[ $# -gt 0 ] && shift || true

INSTALL_ROOT=${DUTY_INSTALL_ROOT:-"$HOME/.local"}

while [ $# -gt 0 ]; do
  case "$1" in
    --install-root)
      INSTALL_ROOT=${2:-}
      [ -n "$INSTALL_ROOT" ] || { echo "--install-root requires a value" >&2; exit 1; }
      shift 2
      ;;
    --install-root=*)
      INSTALL_ROOT=${1#--install-root=}
      shift
      ;;
    -h|--help|help)
      cat <<'EOF'
duty installer

Usage:
  install.sh install [--install-root <path>]
  install.sh upgrade [--install-root <path>]
  install.sh uninstall [--install-root <path>]

Environment:
  DUTY_INSTALL_ROOT  Defaults to $HOME/.local
EOF
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

install_duty() {
  cargo install --locked --path crates/duty-cli --root "$INSTALL_ROOT"
  "$INSTALL_ROOT/bin/duty" --version
  printf 'installed duty to %s\n' "$INSTALL_ROOT/bin/duty"
}

uninstall_duty() {
  rm -f "$INSTALL_ROOT/bin/duty"
  printf 'removed %s\n' "$INSTALL_ROOT/bin/duty"
}

case "$COMMAND" in
  install|upgrade) install_duty ;;
  uninstall) uninstall_duty ;;
  *)
    echo "unknown command: $COMMAND" >&2
    exit 1
    ;;
esac

