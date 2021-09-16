
JQサンプル
```sh
cat 210916173948-test-runner-happy-path.json |jq -cr '[.ts,.msg,.module,.function,.location]|join("\t")' > 210916173948.tmp
```