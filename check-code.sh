#!/bin/bash
# 本地代码质量检查脚本

set -e
cd "$(dirname "$0")"

echo "🔍 运行代码质量检查..."

# 检查参数，如果有参数则只运行指定的检查
CHECK_TYPE="${1:-all}"

run_flake8() {
    echo "📝 运行 flake8 代码风格检查..."
    flake8 --config=testenv/linters/flake8.ini kvmd testenv/tests *.py
}

run_pylint() {
    echo "🔎 运行 pylint 代码质量分析..."
    pylint -j0 --rcfile=testenv/linters/pylint.ini --output-format=colorized --reports=no kvmd testenv/tests *.py || true
}

run_mypy() {
    echo "🔧 运行 mypy 类型检查..."
    mypy --config-file=testenv/linters/mypy.ini --cache-dir=testenv/.mypy_cache kvmd testenv/tests *.py || true
}

run_vulture() {
    echo "💀 运行 vulture 死代码检测..."
    vulture --ignore-names=_format_P,Plugin --ignore-decorators=@exposed_http,@exposed_ws,@pytest.fixture kvmd testenv/tests *.py testenv/linters/vulture-wl.py || true
}

run_eslint() {
    echo "📜 运行 eslint JavaScript检查..."
    if command -v eslint >/dev/null 2>&1; then
        eslint --cache-location=/tmp --config=testenv/linters/eslintrc.js --color web/share/js || true
    else
        echo "⚠️  eslint 未安装，跳过"
    fi
}

run_htmlhint() {
    echo "📄 运行 htmlhint HTML检查..."
    if command -v htmlhint >/dev/null 2>&1; then
        htmlhint --config=testenv/linters/htmlhint.json web/*.html web/*/*.html || true
    else
        echo "⚠️  htmlhint 未安装，跳过"
    fi
}

run_shellcheck() {
    echo "🐚 运行 shellcheck Shell脚本检查..."
    if command -v shellcheck >/dev/null 2>&1; then
        shellcheck --color=always kvmd.install scripts/* || true
    else
        echo "⚠️  shellcheck 未安装，跳过"
    fi
}

case "$CHECK_TYPE" in
    flake8) run_flake8 ;;
    pylint) run_pylint ;;
    mypy) run_mypy ;;
    vulture) run_vulture ;;
    eslint) run_eslint ;;
    htmlhint) run_htmlhint ;;
    shellcheck) run_shellcheck ;;
    all)
        run_flake8
        run_pylint
        run_mypy
        run_vulture
        run_eslint
        run_htmlhint
        run_shellcheck
        ;;
    *)
        echo "用法: $0 [flake8|pylint|mypy|vulture|eslint|htmlhint|shellcheck|all]"
        exit 1
        ;;
esac

echo "✅ 代码质量检查完成！"