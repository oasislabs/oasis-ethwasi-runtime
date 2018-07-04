#!/bin/sh -ex
filter=${1:-copper=evm}
logdir=$(mktemp --directory --tmpdir evm-logs.XXXXXX)
pods=$(kubectl get -o go-template='{{range .items}}{{.metadata.name}}{{"\n"}}{{end}}' -l $filter pods)
for pod in $pods; do
    kubectl logs "$pod" >"$logdir/$pod.log" &
done
wait
less -R "$logdir"/*
rm -r "$logdir"
