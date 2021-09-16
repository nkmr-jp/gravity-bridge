
JQサンプル
```sh
cat 210916145301-test-runner-happy-path.json |jq -cr '[.msg,.module,.function,.location]|join("\t")'| sed -e "s|.dev|.com|g" > 210916145301-test-runner-happy-path-msg-url.log
```