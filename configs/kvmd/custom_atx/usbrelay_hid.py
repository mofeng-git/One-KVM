import sys
import hid

VENDOR_ID = 0x5131
PRODUCT_ID = 0x2007

def find_usbrelay():
    for device in hid.enumerate():
        if device.get("vendor_id") == VENDOR_ID and device.get("product_id") == PRODUCT_ID:
            return device
    return None

def send_command(device_info, channel, onoff):
    device = hid.device()
    device.open(device_info['vendor_id'], device_info['product_id'])
    if device is None:
        print("Failed to open device.")
        return

    try:
        cmd = [0xA0, channel, onoff, 0xA0 + channel + onoff]
        device.write(bytearray(cmd))
    finally:
        device.close()

def main():
    if len(sys.argv) != 3:
        print("Usage:\n"
              "\tpython script.py id on|off")
        return
    
    try:
        id = int(sys.argv[1])
        if sys.argv[2].lower() == 'on':
            onoff = 1
        elif sys.argv[2].lower() == 'off':
            onoff = 0
        else:
            raise ValueError
    except ValueError:
        print("Invalid command, use 'on' or 'off'")
        return
    
    device_info = find_usbrelay()
    if device_info is None:
        print("USB relay not found")
    else:
        send_command(device_info, id, onoff)
        print(f"Sent command to channel {id}: {'ON' if onoff else 'OFF'}")

if __name__ == "__main__":
    main()
