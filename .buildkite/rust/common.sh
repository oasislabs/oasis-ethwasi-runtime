#######################################
# Common initialization for Rust builds
#######################################

source .buildkite/scripts/common.sh

####################
# Set up environment
####################
export RUST_BACKTRACE="1"
export OASIS_UNSAFE_KM_POLICY_KEYS="1"
export OASIS_UNSAFE_SKIP_AVR_VERIFY="1"

##################################
# Set up RUSTFLAGS for the build #
##################################
if [ -z ${RUSTLINT+x} ]; then
    RUSTLINT=""
    for opt in $(cat .buildkite/rust/lint.txt | grep -v '#'); do
        RUSTLINT=$RUSTLINT" -D "$opt
    done

    export RUSTLINT
    if [ -z ${RUSTFLAGS+x} ]; then
        export RUSTFLAGS=$RUSTLINT
    else
        export RUSTFLAGS=$RUSTFLAGS" "$RUSTLINT
    fi

    echo "Using RUSTFLAGS="$RUSTFLAGS
fi
