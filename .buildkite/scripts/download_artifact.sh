#!/bin/bash

################################################################################
# A script to download artifacts from an external buildkite pipeline.
#
# Uses Buildkite's "List all builds" API (with the appropriate query parameters)
# to retrieve the raw URL to download the artifact.
#
# See https://buildkite.com/docs/apis/rest-api/builds#list-all-builds
#
# ./download_artifact.sh <PIPELINE> <BRANCH> <JOB_NAME> <ARTIFACT_NAME> <OUTPUT_DIR>
#
# Required Args:
#
# - PIPELINE:      The name of the pipeline (e.g., e2e-tests).
# - BRANCH:        The branch from which to grab the latest version of the artifact.
# - JOB_NAME:      The "label" given to the buildkite step.
# - ARTIFACT_NAME: The name of the artifact to download (e.g., build.zip).
# - OUTPUT_DIR:    The desired destination directory for the artifact to live.
#
################################################################################

set -euo pipefail

ORGANIZATION="oasislabs"

BUILDKITE_ACCESS_TOKEN=${BUILDKITE_ACCESS_TOKEN:-$(cat ~/.buildkite/read_only_buildkite_api_access_token)}

################################################################################
# Required arguments.
################################################################################

# The name of the pipeline to download from.
PIPELINE=$1
# The branch of the pipeline to download from.
BRANCH=$2
# The job name within the pipeline responsible for uploading the artifact.
# This is the "label" specified in the pipeline's definition.
JOB_NAME=$3
# The name of the artifact we want to download.
ARTIFACT_NAME=$4
# The output filename of the artifact
OUTPUT_DIR=$5

################################################################################
# First let's fetch the correct build.
################################################################################

# Query param ensuring we only fetch builds that passed.
STATE="passed"
# Query request to be issued.
BUILDS_QUERY="https://api.buildkite.com/v2/organizations/$ORGANIZATION/pipelines/$PIPELINE/builds?state=$STATE&branch=$BRANCH&access_token=$BUILDKITE_ACCESS_TOKEN"
# All recent builds passing our query.
BUILDS_ARRAY=$(curl $BUILDS_QUERY)
# Take the first build given (since it's the latest).
BUILD=$(echo $BUILDS_ARRAY | jq '.[0]')

################################################################################
# Now let's parse the jobs in the build to get all the ARTIFACTs.
################################################################################

# Extract url to query the artifacts associated with the given job.
QUOTED_ARTIFACTS_URL=$(echo $BUILD | jq '.jobs | first(.[] | if .name == "'"${JOB_NAME}"'" then .artifacts_url else empty end)')
# Remove quotes so we can append the access token.
ARTIFACTS_URL=$(echo "$QUOTED_ARTIFACTS_URL" | tr -d '"')
# Append the access token to finish constructing the URL.
ARTIFACTS_QUERY=$ARTIFACTS_URL?access_token=$BUILDKITE_ACCESS_TOKEN
# Fetch the artifacts
ARTIFACTS_ARRAY=$(curl $ARTIFACTS_QUERY)

################################################################################
# Extract the correct ARTIFACT and extract its aws S3 url.
################################################################################

# Extract the url to download the correct artifact.
QUOTED_ARTIFACT_DOWNLOAD_URL=$(echo $ARTIFACTS_ARRAY | jq 'first(.[] | if .filename == "'"${ARTIFACT_NAME}"'" then .download_url else empty end)')
# Remove quotes so we can append the access token.
ARTIFACT_DOWNLOAD_URL=$(echo "$QUOTED_ARTIFACT_DOWNLOAD_URL" | tr -d '"')
# Construct the query.
QUERY_DOWNLOAD_URL=$ARTIFACT_DOWNLOAD_URL?access_token=$BUILDKITE_ACCESS_TOKEN
# Issue the request and extract the S3 url.
QUOTED_RAW_S3_DOWNLOAD_URL=$(curl $QUERY_DOWNLOAD_URL | jq '.url')
# Remove quotes
RAW_S3_DOWNLOAD_URL=$(echo "$QUOTED_RAW_S3_DOWNLOAD_URL" | tr -d '"')

################################################################################
# Execute the final download.
################################################################################

curl ${RAW_S3_DOWNLOAD_URL} --output "$OUTPUT_DIR/$ARTIFACT_NAME"
