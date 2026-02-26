#!/usr/bin/env bash

SETTINGS="$HOME/.claude/settings.json"

HOOKS='{
  "hooks": {
    "Notification": [{
      "hooks": [{
        "type": "command",
        "command": "zellij pipe -n \"notify\" -a \"pane_id=$ZELLIJ_PANE_ID\" \"notification\""
      }]
    }],
    "Stop": [{
      "hooks": [{
        "type": "command",
        "command": "zellij pipe -n \"notify\" -a \"pane_id=$ZELLIJ_PANE_ID\" \"stop\""
      }]
    }],
    "PostToolUse": [{
      "matcher": "*",
      "hooks": [{
        "type": "command",
        "command": "zellij pipe -n \"notify\" -a \"pane_id=$ZELLIJ_PANE_ID\" \"posttooluse\""
      }]
    }]
  }
}'

if [ ! -f "$SETTINGS" ]; then
  mkdir -p "$(dirname "$SETTINGS")"
  echo "$HOOKS" > "$SETTINGS"
  echo "✅ Created $SETTINGS with hooks"
  exit 0
fi

if grep -q "Notification" "$SETTINGS"; then
  echo "✅ Hooks already configured"
  exit 0
fi

if ! command -v jq &> /dev/null; then
  echo "❌ jq required: brew install jq"
  exit 1
fi

jq -s '.[0] * .[1]' "$SETTINGS" <(echo "$HOOKS") > "$SETTINGS.tmp"
mv "$SETTINGS.tmp" "$SETTINGS"
echo "✅ Merged hooks into $SETTINGS"
