
JQサンプル
```sh
cat 210917182011-test-runner-happy-path.json.json |jq -cr '[.ts,.level,.module,.function,.location,.msg]|join("\t")' | pbcopy
```