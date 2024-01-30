#!/bin/bash

# 存储 stream.sh 的进程 ID
b_pid=""


cleanup() {
    echo "Received SIGINT. Terminating the process group..."
    [ -n "$b_pid" ] && pkill -9 -g $b_pid  # 终止整个进程组
    exit 0
}

# 捕获 SIGINT 信号
trap cleanup SIGINT


while true; do
    # 检测是否有包含 "ustreamer" 的进程
    if pgrep -f "/usr/bin/ustreamer " > /dev/null; then
        # 如果存在，但是 stream.sh 进程不存在，执行 stream.sh 并记录其进程 ID
        if [ -z "$b_pid" ]; then
            echo "Found a process with 'ustreamer' in the command. Executing stream.sh in the background..."
            setsid /usr/share/kvmd/stream.sh &
            b_pid=$(ps -o pgid= $!)
            echo "stream.sh started with PID: $b_pid"
        else
            echo "Process with 'ustreamer' is already running. Skipping..."
        fi
    else
        # 如果不存在 "ustreamer" 进程，但是 stream.sh 进程存在，终止 stream.sh 并清除进程 ID
        if [ -n "$b_pid" ]; then
            echo "No process with 'ustreamer' found. Terminating stream.sh (PID: $b_pid)..."
            pkill -9 -g $b_pid
            b_pid=""
        else
            echo "No process with 'ustreamer' found. Waiting for the next check..."
        fi
    fi

    # 等待一段时间，可以根据需要调整等待的时间间隔
    sleep 1  

done
