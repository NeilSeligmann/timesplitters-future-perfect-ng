# T3863: sourced from the repository root only when timing is requested.
# Keep paths absolute when Cargo runs from its crate directory.
build_stage_tool=$(pwd)/tools/build_stages.py
build_stage_ninja_log=$(pwd)/$dir/.ninja_log
case $TSFP_BUILD_STAGE_RECEIPT in
    /*) ;;
    *) TSFP_BUILD_STAGE_RECEIPT=$(pwd)/$TSFP_BUILD_STAGE_RECEIPT ;;
esac
export TSFP_BUILD_STAGE_RECEIPT

build_stage() {
    build_stage_name=$1
    shift
    if [ "$build_stage_name" = host ]; then
        python3 "$build_stage_tool" --receipt "$TSFP_BUILD_STAGE_RECEIPT" \
            --stage "$build_stage_name" --ninja-log "$build_stage_ninja_log" -- "$@"
    else
        python3 "$build_stage_tool" --receipt "$TSFP_BUILD_STAGE_RECEIPT" \
            --stage "$build_stage_name" -- "$@"
    fi
}
