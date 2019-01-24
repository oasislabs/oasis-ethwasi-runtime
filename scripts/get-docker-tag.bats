#!/usr/bin/env bats

##
# Test file for get-docker-tags.sh
#
# If you are not familiar, check out
# Bash Automated Testing System (BATS):
# https://github.com/bats-core/bats-core
##

@test "Provide zero arguments should error" {
  run ./get-docker-tag.sh
  [ "$status" -eq 1 ]
  [[ "$output" =~ "\$1: unbound variable" ]]
}

@test "Provide only git_branch should succeed" {
  run ./get-docker-tag.sh 'some_git_branch_name'
  [ "$status" -eq 0 ]
}

@test "Provide git_branch and git_tag_name should succeed" {
  run ./get-docker-tag.sh 'some_git_branch_name' 'some_git_tag_name'
  [ "$status" -eq 0 ]
}

@test "Provide branch and tag: should use tag as prefix" {
  run ./get-docker-tag.sh 'master' 'some_tag_name'
  [ "$status" -eq 0 ]
  [[ "$output" =~ "some_tag_name-" ]]
}

@test "Provide branch and no tag: should use branch as prefix" {
  run ./get-docker-tag.sh 'beta'
  [ "$status" -eq 0 ]
  [[ "$output" =~ "beta-" ]]
}
