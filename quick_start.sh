#!/bin/bash
#Install Latest Stable One-KVM Dcoker Release

DOCKER_IMAGE_PATH="registry.cn-hangzhou.aliyuncs.com/silentwind/kvmd"
DOCKER_PORT="-p 8080:8080 -p 4430:4430 -p 5900:5900 -p 623:623"
DOCKER_NAME="kvmd"
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m'

function check_os_architecture(){
    osCheck=$(uname -a)
    if [[ $osCheck =~ 'x86_64' ]];then
        architecture="amd64"
    elif [[ $osCheck =~ 'arm64' ]] || [[ $osCheck =~ 'aarch64' ]];then
        architecture="arm64"
    elif [[ $osCheck =~ 'armv7l' ]];then
        architecture="armv7l"
    else
        echo "暂不支持的系统架构，请参阅官方文档，选择受支持的系统。\n退出程序"
        exit 1
    fi
}

function check_docker_exists() {
    if command -v docker &> /dev/null; then
        echo "$(docker -v)"
    else
        echo "Docker 未安装,退出程序"
        exit 1
    fi
}

function check_sudo_exists() {
    if command -v sudo > /dev/null 2>&1; then
        sudo_command="sudo"
    else
        sudo_command=""
    fi
}

function delete_kvmd_container(){
    if docker ps -a --format '{{.Names}}' | grep -q '^kvmd$'; then
        $sudo_command docker stop $DOCKER_NAME
        $sudo_command docker rm $DOCKER_NAME
    fi
}

function check_otg_device(){
    $sudo_command modprobe libcomposite > /dev/null|| echo -e "${YELLOW}libcomposite 内核模块加载失败${NC}"
    if [[ "$architecture" != "amd64" ]] && [[ -d "/sys/class/udc" ]]; then
        if [[ "$(ls -A /sys/class/udc)" ]] || [[ "$(ls -A /sys/class/usb_role)" ]]; then
            otg_devices=$(ls -A /sys/class/udc)
            otg_status=$(cat /sys/class/usb_role/*/role 2>/dev/null | head -n 1)
            echo -e "${GREEN}当前系统支持 OTG：$otg_devices OTG 状态：$otg_status${NC}"
        fi
    else
        echo -e "${RED}当前系统不支持 OTG，退出程序${NC}"
        exit 1
    fi
    if [[ ! -d "/sys/kernel/config" ]];then
        echo -e "${RED}当前系统不支持 configfs 文件系统，退出程序${NC}"
        exit 1
    fi
}

function check_video_device(){
    if ls /dev/video* 1> /dev/null 2>&1; then
        video_devices=($(ls /dev/video* 2>/dev/null))
        video_num_devices=${#video_devices[@]}
        echo -e ""${GREEN}找到视频设备：$(ls -A /dev/video*)${NC}""
    else
        echo -e "${RED}未找到任何视频采集设备，退出程序${NC}"
        exit 1
    fi
}

function check_repeat_install(){
    if docker ps -a --format '{{.Names}}' | grep -q '^kvmd$'; then
        echo -e "${YELLOW}检查到 kvmd 容器已存在，是否删除容器重新部署？${NC}"
        read -p "y/n: " delete_choice
        case $delete_choice in
            y|Y)
                delete_kvmd_container
                ;;
            n|N)
                echo -e "${RED}退出程序${NC}"
                exit 1
                ;;
            *)
                echo -e "${RED}无效的选择，请输入 y 或者 n，退出程序${NC}"
                exit 1
                ;;
        esac
    fi
    if [[ -d "kvmd_config" ]]; then
        echo -e "${YELLOW}检查到此前配置文件夹已存在，是否删除此前配置文件夹？${NC}"
        read -p "y/n: " delete_choice
        case $delete_choice in
            y|Y)
                $sudo_command rm -r kvmd_config
                ;;
            n|N)
                echo -e ""
                ;;
            *)
                echo -e "${RED}无效的选择，请输入 y 或者 n，退出程序${NC}"
                exit 1
                ;;
        esac
    fi
}

function show_main_menu() {
    echo -e "${BLUE}==============================${NC}"
    echo -e "${BLUE}     One-KVM Docker 版管理     ${NC}"
    echo -e "${BLUE}==============================${NC}"

    echo " 1. 安装 One-KVM Docker 版"
    echo ""
    echo " 2. 卸载 One-KVM Docker 版"
    echo ""
    echo " 3. 拉取 One-KVM 最新镜像"
    echo ""
    echo " 4. 更多信息"

    echo -e "${BLUE}==============================${NC}"
    read -p "请输入数字（1-4）: " choice
    while [[ "$choice" != "1" && "$choice" != "2" && "$choice" != "3" && "$choice" != "4" ]]; do
        echo -e "${RED}无效的选择，请输入1-4${NC}"
        read -p "请输入数字（1-4）: " choice
    done
    case $choice in
        1)
            check_repeat_install
            get_hid_info
            get_video_info
            get_audio_info
            get_userinfo
            get_userenv
            show_install_info
            get_install_command
            execute_command
            ;;
        2)
            delete_kvmd_container
            ;;
        3)
            $sudo_command docker pull $DOCKER_IMAGE_PATH
            ;;
        4)
            echo -e "${BLUE}作者：${NC}\t\t默风SilentWind"
            echo -e "${BLUE}文档：${NC}\t\thttps://one-kvm.mofeng.run/"
            echo -e "${BLUE}Github：${NC}\thttps://github.com/mofeng-git/One-KVM"
            ;;
        *)
            echo -e "${RED}无效的选择，请输入1-4之间的数字，退出程序${NC}"
            exit 1
            ;;
    esac
}

function get_hid_info() {
    if [[ "$architecture" == "amd64" ]]; then
        echo -e "${GREEN}使用的 HID 硬件类型：CH9329${NC}"
        use_hid="CH9329"
    else
        echo -e "${GREEN}请选择使用的 HID 硬件类型：${NC}"
        echo " 1. OTG"
        echo " 2. CH9329"
        read -p "请输入数字（1 或 2）: " hardware_type
        while [[ "$hardware_type" != "1" && "$hardware_type" != "2" ]]; do
            echo -e "${RED}无效的选择，请输入1或2。${NC}"
            read -p "请输入数字（1 或 2）: " hardware_type
        done
        if [[ "$hardware_type" == "1" ]]; then
            use_hid="OTG"
        else
            use_hid="CH9329"
        fi
    fi

    if [[ "$use_hid" == "CH9329" ]]; then
        if ls /dev/ttyUSB* 1> /dev/null 2>&1; then
            echo -e ""${GREEN}找到串口设备：$(ls -A /dev/ttyUSB*)${NC}""
        else
            echo -e "${RED}未找到任何 USB 串口设备，退出程序${NC}"
            exit 1
        fi
        read -p "请输入 CH9329 硬件的地址（回车使用默认值 /dev/ttyUSB0）: " ch9329_address
        read -p "请输入 CH9329 硬件的波特率（回车使用默认值 9600）: " ch9329_serial_rate
        ch9329_address=${ch9329_address:-/dev/ttyUSB0}
        ch9329_serial_rate=${ch9329_serial_rate:-9600}
    fi

    if [[ "$use_hid" == "OTG" ]]; then
        check_otg_device
    fi
}

function get_video_info() {
    check_video_device
    if [[ "$video_num_devices" == "3" ]]; then
        video_default_device="/dev/video1"
        echo -e "${YELLOW}经检测 /dev/video0 可能不可用，建议使用 /dev/video1${NC}"
    else
        video_default_device="/dev/video0"
    fi
    read -p "请输入视频设备路径（回车使用默认值 $video_default_device）: " video_device
    if [[ -z "$video_device" ]]; then
        video_device=$video_default_device
    fi
}

function get_audio_info() {
    if [[ -d "/dev/snd" ]]; then
        echo -e ""${GREEN}找到音频设备：$(ls -A /dev/snd)${NC}""
        read -p "请输入音频设备路径（回车使用默认值 hw:0）: " audio_device
        if [[ -z "$audio_device" ]]; then
            audio_device="hw:0"
        fi
    else
        echo -e "${YELLOW}未找到任何音频采集设备${NC}"
        audio_device="none"
    fi
}

function get_userinfo() {
    read -p "请输入用户名（回车使用默认值 admin）: " username
    read -s -p "请输入密码（回车使用默认值 admin）: " password
    if [[ -z "$username" ]]; then
        username="admin"
    fi
    if [[ -z "$password" ]]; then
        password="admin"
    fi
}

function get_userenv() {
    echo -e "\n"
    read -p "额外用户环境变量（回车则留空）: " userenv
}

function show_install_info() {
    echo -e "\n\n${BLUE}==============================${NC}"
    echo -e "${BLUE}安装信息总览：${NC}"
    if [[ "$use_hid" == "CH9329" ]]; then
        echo -e "CH9329 设备: \t${GREEN}$ch9329_address${NC} \tCH9329 波特率: \t${GREEN}$ch9329_serial_rate${NC}"
    fi
    if [[ "$use_hid" == "OTG" ]]; then
        echo -e "OTG端口：\t${GREEN}$otg_devices${NC} \tOTG 状态：\t${GREEN}$otg_status${NC}"
    fi
    echo -e "视频设备: \t${GREEN}$video_device${NC} \t音频设备: \t${GREEN}$audio_device${NC}"
    echo -e "用户名: \t${GREEN}$username${NC} \t\t密码: \t${GREEN}$password${NC}"
}

function get_install_command(){
    local docker_init_command="docker run -itd --name $DOCKER_NAME"
    local append_command=""
    local append_env=""

    if [[ "$use_hid" == "CH9329" ]]; then
        append_command="--device $video_device:/dev/video0 --device $ch9329_address:/dev/ttyUSB0 -v ./kvmd_config:/etc/kvmd"
        
        if [[ -d "/dev/snd" ]]; then
            append_command="$append_command --device /dev/snd:/dev/snd -e AUDIONUM=${audio_device:3}"
        fi
        append_env="-e USERNAME=$username -e PASSWORD=$password -e CH9329SPEED=$ch9329_serial_rate"
        docker_command="$sudo_command $docker_init_command $append_command $DOCKER_PORT $append_env $userenv $DOCKER_IMAGE_PATH"
    else
        append_command="--privileged=true -v /lib/modules:/lib/modules:ro -v /dev:/dev -v /sys/kernel/config:/sys/kernel/config -v ./kvmd_config:/etc/kvmd"
        if [[ -d "/dev/snd" ]]; then
            append_command="$append_command -e AUDIONUM=${audio_device:3}"
        fi
        append_env="-e OTG=1 -e USERNAME=$username -e PASSWORD=$password -e VIDEONUM=${video_device:10} -e AUDIONUM=${audio_device:3}"
        docker_command="$sudo_command $docker_init_command $append_command $DOCKER_PORT $append_env $userenv $DOCKER_IMAGE_PATH"
    fi
    echo -e "\n${BLUE}Docker 部署命令：${NC}\n$docker_command"
    echo -e "${BLUE}==============================${NC}\n"
}

function execute_command(){
    echo -e "${BLUE}One-KVM 部署中......${NC}"
    eval "$docker_command"
    local exit_status=$?
    if [[ $exit_status -eq 0 ]]; then
        echo -e "${BLUE}One-KVM 部署成功${NC}"
        $sudo_command docker update --restart=always $DOCKER_NAME
        if [[ "$use_hid" == "OTG" ]]; then
           execute_otg_command
        fi
    else
        echo -e "${RED}One-KVM 部署失败，退出状态码为 $exit_status${NC}"
    fi
}

function execute_otg_command(){
    $sudo_command echo "device" > /sys/class/usb_role/**/role || echo -e "${YELLOW}OTG 端口切换 device 模式失败${NC}"
    if grep -q "usb_role" /etc/rc.local; then
        echo -e ""
    else
        $sudo_command sed -i '/^exit 0/i echo device > \/sys\/class\/usb_role\/\*\*\/role' /etc/rc.local
        $sudo_command chmod +x /etc/rc.local
    fi
    if grep -q "libcomposite" /etc/modules.conf; then
        echo -e ""
    else
        $sudo_command echo "libcomposite" >> /etc/modules.conf
    fi
}

check_os_architecture
check_docker_exists
check_sudo_exists
show_main_menu