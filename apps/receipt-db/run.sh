#!/usr/bin/env bash
# tmux のセッションから呼ばれる。KICKOFF.md を最初の指示にして Claude Code を起動する。
cd "$(dirname "$0")" || exit 1
${CLAUDE_CMD:-claude} "$(cat KICKOFF.md)"
# Claude Code を終了してもセッションが閉じないよう、シェルに戻す
exec bash
