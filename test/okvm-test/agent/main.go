//go:build windows

package main

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"flag"
	"fmt"
	"log"
	"math/rand"
	"net/http"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"syscall"
	"time"
	"unsafe"

	"github.com/gorilla/websocket"
)

const (
	wsOverlapped   = 0x00000000
	wsPopup        = 0x80000000
	wsVisible      = 0x10000000
	wsExTopmost    = 0x00000008
	wsExToolWindow = 0x00000080

	cwUseDefault = 0x80000000
	swShow       = 5
	swHide       = 0

	wmDestroy     = 0x0002
	wmPaint       = 0x000F
	wmClose       = 0x0010
	wmKeyDown     = 0x0100
	wmKeyUp       = 0x0101
	wmChar        = 0x0102
	wmSysKeyDown  = 0x0104
	wmSysKeyUp    = 0x0105
	wmTimer       = 0x0113
	wmMouseMove   = 0x0200
	wmLButtonDown = 0x0201
	wmLButtonUp   = 0x0202
	wmRButtonDown = 0x0204
	wmRButtonUp   = 0x0205
	wmMouseWheel  = 0x020A

	vkLShift   = 0xA0
	vkRShift   = 0xA1
	vkLControl = 0xA2
	vkRControl = 0xA3
	vkLMenu    = 0xA4
	vkRMenu    = 0xA5
	vkLWin     = 0x5B
	vkRWin     = 0x5C

	modLeftCtrl   = 0x01
	modLeftShift  = 0x02
	modLeftAlt    = 0x04
	modLeftMeta   = 0x08
	modRightCtrl  = 0x10
	modRightShift = 0x20
	modRightAlt   = 0x40
	modRightMeta  = 0x80

	wmApp             = 0x8000
	wmAppInvalidate   = wmApp + 1
	wmAppShow         = wmApp + 2
	wmAppHide         = wmApp + 3
	wmAppFocus        = wmApp + 4
	wmAppDynamicStart = wmApp + 5
	wmAppDynamicStop  = wmApp + 6

	dynamicTimerID = 1

	colorWindow = 5

	driveUnknown   = 0
	driveNoRootDir = 1
	driveRemovable = 2
	driveFixed     = 3

	genericRead            = 0x80000000
	fileShareRead          = 0x00000001
	fileShareWrite         = 0x00000002
	openExisting           = 3
	fileAttributeNormal    = 0x00000080
	fileFlagNoBuffering    = 0x20000000
	fileFlagSequentialScan = 0x08000000
)

var (
	user32   = syscall.NewLazyDLL("user32.dll")
	kernel32 = syscall.NewLazyDLL("kernel32.dll")
	gdi32    = syscall.NewLazyDLL("gdi32.dll")
	imm32    = syscall.NewLazyDLL("imm32.dll")

	procRegisterClassExW      = user32.NewProc("RegisterClassExW")
	procCreateWindowExW       = user32.NewProc("CreateWindowExW")
	procDefWindowProcW        = user32.NewProc("DefWindowProcW")
	procDispatchMessageW      = user32.NewProc("DispatchMessageW")
	procGetMessageW           = user32.NewProc("GetMessageW")
	procTranslateMessage      = user32.NewProc("TranslateMessage")
	procPostMessageW          = user32.NewProc("PostMessageW")
	procPostQuitMessage       = user32.NewProc("PostQuitMessage")
	procShowWindow            = user32.NewProc("ShowWindow")
	procSetForegroundWindow   = user32.NewProc("SetForegroundWindow")
	procGetSystemMetrics      = user32.NewProc("GetSystemMetrics")
	procInvalidateRect        = user32.NewProc("InvalidateRect")
	procUpdateWindow          = user32.NewProc("UpdateWindow")
	procSetTimer              = user32.NewProc("SetTimer")
	procKillTimer             = user32.NewProc("KillTimer")
	procGetKeyState           = user32.NewProc("GetKeyState")
	procBeginPaint            = user32.NewProc("BeginPaint")
	procEndPaint              = user32.NewProc("EndPaint")
	procFillRect              = user32.NewProc("FillRect")
	procCreateSolidBrush      = gdi32.NewProc("CreateSolidBrush")
	procDeleteObject          = gdi32.NewProc("DeleteObject")
	procImmAssociateContext   = imm32.NewProc("ImmAssociateContext")
	procGetModuleHandleW      = kernel32.NewProc("GetModuleHandleW")
	procQueryPerformanceCount = kernel32.NewProc("QueryPerformanceCounter")
	procQueryPerformanceFreq  = kernel32.NewProc("QueryPerformanceFrequency")
	procGetLogicalDrives      = kernel32.NewProc("GetLogicalDrives")
	procGetDriveTypeW         = kernel32.NewProc("GetDriveTypeW")
	procGetVolumeInformationW = kernel32.NewProc("GetVolumeInformationW")
	procGetDiskFreeSpaceExW   = kernel32.NewProc("GetDiskFreeSpaceExW")
	procGetDiskFreeSpaceW     = kernel32.NewProc("GetDiskFreeSpaceW")
	procCreateFileW           = kernel32.NewProc("CreateFileW")
	procReadFile              = kernel32.NewProc("ReadFile")
	procCloseHandle           = kernel32.NewProc("CloseHandle")
	procSetProcessDPIAware    = user32.NewProc("SetProcessDPIAware")
)

type wndClassEx struct {
	Size       uint32
	Style      uint32
	WndProc    uintptr
	ClsExtra   int32
	WndExtra   int32
	Instance   uintptr
	Icon       uintptr
	Cursor     uintptr
	Background uintptr
	MenuName   *uint16
	ClassName  *uint16
	IconSm     uintptr
}

type point struct {
	X int32
	Y int32
}

type msg struct {
	HWnd    uintptr
	Message uint32
	WParam  uintptr
	LParam  uintptr
	Time    uint32
	Pt      point
}

type rect struct {
	Left   int32
	Top    int32
	Right  int32
	Bottom int32
}

type paintStruct struct {
	Hdc         uintptr
	Erase       int32
	RcPaint     rect
	Restore     int32
	IncUpdate   int32
	RgbReserved [32]byte
}

type appState struct {
	mu                  sync.Mutex
	hwnd                uintptr
	bgColor             uint32
	colorHex            string
	lastColorChangeQpc  int64
	lastColorChangeUnix int64
	colorSequence       int64
	dynamicActive       bool
	dynamicFPS          int
	dynamicFrame        int64
	events              []hidEvent
}

type hidEvent struct {
	Type        string `json:"type"`
	Code        uint32 `json:"code,omitempty"`
	Char        string `json:"char,omitempty"`
	Modifiers   uint32 `json:"modifiers,omitempty"`
	X           int32  `json:"x,omitempty"`
	Y           int32  `json:"y,omitempty"`
	WheelDelta  int16  `json:"wheel_delta,omitempty"`
	Qpc         int64  `json:"qpc"`
	UnixNano    int64  `json:"unix_nano"`
	Description string `json:"description,omitempty"`
}

type command struct {
	ID      string          `json:"id"`
	Command string          `json:"command"`
	Payload json.RawMessage `json:"payload"`
}

type response struct {
	ID      string      `json:"id"`
	OK      bool        `json:"ok"`
	Type    string      `json:"type,omitempty"`
	Payload interface{} `json:"payload,omitempty"`
	Error   string      `json:"error,omitempty"`
}

type driveInfo struct {
	Letter       string `json:"letter"`
	Root         string `json:"root"`
	Type         uint32 `json:"type"`
	Label        string `json:"label,omitempty"`
	FileSystem   string `json:"file_system,omitempty"`
	TotalBytes   uint64 `json:"total_bytes,omitempty"`
	FreeBytes    uint64 `json:"free_bytes,omitempty"`
	Removable    bool   `json:"removable"`
	WriteCapable bool   `json:"write_capable"`
	ObservedNano int64  `json:"observed_unix_nano"`
}

var state = &appState{
	bgColor:             0x00101010,
	colorHex:            "#101010",
	lastColorChangeQpc:  qpcNow(),
	lastColorChangeUnix: time.Now().UnixNano(),
}

func main() {
	listen := flag.String("listen", "0.0.0.0:8765", "listen address")
	noWindow := flag.Bool("no-window", true, "start with the test window hidden; commands show it when needed")
	flag.Parse()

	procSetProcessDPIAware.Call()

	uiReady := make(chan struct{})
	go runUI(uiReady, *noWindow)
	<-uiReady

	log.Printf("Windows agent listening on ws://%s/agent", *listen)
	if err := serveAgent(*listen); err != nil {
		log.Fatal(err)
	}
}

var upgrader = websocket.Upgrader{
	CheckOrigin: func(r *http.Request) bool {
		return true
	},
}

func serveAgent(listen string) error {
	http.HandleFunc("/agent", handleAgentWebSocket)
	return http.ListenAndServe(listen, nil)
}

func handleAgentWebSocket(w http.ResponseWriter, r *http.Request) {
	conn, err := upgrader.Upgrade(w, r, nil)
	if err != nil {
		log.Printf("websocket upgrade failed: %v", err)
		return
	}
	defer conn.Close()

	hello := map[string]interface{}{
		"type":       "hello",
		"hostname":   hostname(),
		"qpc_freq":   qpcFreq(),
		"drives":     listDrives(),
		"screen":     screenInfo(),
		"agent_time": time.Now().UnixNano(),
	}
	if err := conn.WriteJSON(hello); err != nil {
		log.Printf("failed to send hello: %v", err)
		return
	}

	for {
		var cmd command
		if err := conn.ReadJSON(&cmd); err != nil {
			log.Printf("agent client disconnected: %v", err)
			return
		}
		resp := handleCommand(cmd)
		if err := conn.WriteJSON(resp); err != nil {
			log.Printf("failed to send command response: %v", err)
			return
		}
	}
}

func handleCommand(cmd command) response {
	defer func() {
		if r := recover(); r != nil {
			log.Printf("panic while handling %s: %v", cmd.Command, r)
		}
	}()

	switch cmd.Command {
	case "ping":
		return ok(cmd, map[string]interface{}{
			"qpc":       qpcNow(),
			"unix_nano": time.Now().UnixNano(),
		})
	case "show":
		var p struct {
			Color string `json:"color"`
			Full  bool   `json:"full"`
		}
		_ = json.Unmarshal(cmd.Payload, &p)
		if p.Color == "" {
			p.Color = "#00ff00"
		}
		display := setColor(p.Color)
		showWindow(true)
		focusWindow()
		return ok(cmd, display)
	case "start_dynamic":
		var p struct {
			FPS int `json:"fps"`
		}
		_ = json.Unmarshal(cmd.Payload, &p)
		if p.FPS <= 0 {
			p.FPS = 60
		}
		display := startDynamic(p.FPS)
		showWindow(true)
		focusWindow()
		return ok(cmd, display)
	case "stop_dynamic":
		stopDynamic()
		return ok(cmd, currentDisplayState())
	case "schedule_color":
		var p struct {
			Color   string `json:"color"`
			DelayMS int    `json:"delay_ms"`
		}
		_ = json.Unmarshal(cmd.Payload, &p)
		if p.Color == "" {
			p.Color = "#00ff00"
		}
		if p.DelayMS < 0 {
			p.DelayMS = 0
		}
		delay := time.Duration(p.DelayMS) * time.Millisecond
		scheduledUnix := time.Now().Add(delay).UnixNano()
		color := normalizeColor(p.Color)
		time.AfterFunc(delay, func() {
			setColor(color)
			showWindow(true)
		})
		showWindow(true)
		focusWindow()
		return ok(cmd, map[string]interface{}{
			"color":               color,
			"delay_ms":            p.DelayMS,
			"scheduled_unix_nano": scheduledUnix,
			"qpc":                 qpcNow(),
			"unix_nano":           time.Now().UnixNano(),
		})
	case "display_state":
		return ok(cmd, currentDisplayState())
	case "hide":
		stopDynamic()
		showWindow(false)
		return ok(cmd, map[string]interface{}{"hidden": true})
	case "begin_hid_capture":
		clearEvents()
		showWindow(true)
		focusWindow()
		return ok(cmd, map[string]interface{}{
			"qpc":       qpcNow(),
			"unix_nano": time.Now().UnixNano(),
		})
	case "get_hid_events":
		return ok(cmd, map[string]interface{}{"events": snapshotEvents()})
	case "msd_snapshot":
		return ok(cmd, map[string]interface{}{"drives": listDrives()})
	case "msd_wait_new":
		var p struct {
			Known     []string `json:"known"`
			TimeoutMS int      `json:"timeout_ms"`
		}
		_ = json.Unmarshal(cmd.Payload, &p)
		if p.TimeoutMS <= 0 {
			p.TimeoutMS = 30000
		}
		drive, err := waitNewDrive(p.Known, time.Duration(p.TimeoutMS)*time.Millisecond)
		if err != nil {
			return fail(cmd, err)
		}
		return ok(cmd, drive)
	case "msd_write_read":
		var p struct {
			Root      string `json:"root"`
			Filename  string `json:"filename"`
			SizeBytes int    `json:"size_bytes"`
		}
		_ = json.Unmarshal(cmd.Payload, &p)
		if p.SizeBytes <= 0 {
			p.SizeBytes = 1024 * 1024
		}
		if p.Filename == "" {
			p.Filename = "okvm-msd-test.bin"
		}
		result, err := writeReadVerify(p.Root, p.Filename, p.SizeBytes)
		if err != nil {
			return fail(cmd, err)
		}
		return ok(cmd, result)
	case "msd_wait_removed":
		var p struct {
			Root      string `json:"root"`
			TimeoutMS int    `json:"timeout_ms"`
		}
		_ = json.Unmarshal(cmd.Payload, &p)
		if p.TimeoutMS <= 0 {
			p.TimeoutMS = 30000
		}
		err := waitRemoved(p.Root, time.Duration(p.TimeoutMS)*time.Millisecond)
		if err != nil {
			return fail(cmd, err)
		}
		return ok(cmd, map[string]interface{}{"removed": true})
	default:
		return fail(cmd, fmt.Errorf("unknown command: %s", cmd.Command))
	}
}

func ok(cmd command, payload interface{}) response {
	return response{ID: cmd.ID, OK: true, Type: cmd.Command, Payload: payload}
}

func fail(cmd command, err error) response {
	return response{ID: cmd.ID, OK: false, Type: cmd.Command, Error: err.Error()}
}

func runUI(ready chan<- struct{}, hidden bool) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	instance := getModuleHandle()
	className := syscall.StringToUTF16Ptr("OKVMWinAgentWindow")
	wndProc := syscall.NewCallback(windowProc)
	wc := wndClassEx{
		Size:       uint32(unsafe.Sizeof(wndClassEx{})),
		WndProc:    wndProc,
		Instance:   instance,
		Background: colorWindow + 1,
		ClassName:  className,
	}
	procRegisterClassExW.Call(uintptr(unsafe.Pointer(&wc)))

	width, height := screenSize()
	style := uintptr(wsPopup)
	if !hidden {
		style |= wsVisible
	}
	hwnd, _, err := procCreateWindowExW.Call(
		wsExTopmost|wsExToolWindow,
		uintptr(unsafe.Pointer(className)),
		uintptr(unsafe.Pointer(syscall.StringToUTF16Ptr("One-KVM Test Agent"))),
		style,
		0,
		0,
		uintptr(width),
		uintptr(height),
		0,
		0,
		instance,
		0,
	)
	if hwnd == 0 {
		log.Fatalf("CreateWindowExW failed: %v", err)
	}
	state.mu.Lock()
	state.hwnd = hwnd
	state.mu.Unlock()
	disableIME(hwnd)
	if hidden {
		showWindow(false)
	} else {
		showWindow(true)
	}
	close(ready)

	var m msg
	for {
		ret, _, _ := procGetMessageW.Call(uintptr(unsafe.Pointer(&m)), 0, 0, 0)
		if int32(ret) <= 0 {
			return
		}
		procTranslateMessage.Call(uintptr(unsafe.Pointer(&m)))
		procDispatchMessageW.Call(uintptr(unsafe.Pointer(&m)))
	}
}

func windowProc(hwnd uintptr, message uintptr, wParam, lParam uintptr) uintptr {
	msg := uint32(message)
	switch msg {
	case wmPaint:
		var ps paintStruct
		hdc, _, _ := procBeginPaint.Call(hwnd, uintptr(unsafe.Pointer(&ps)))
		state.mu.Lock()
		color := state.bgColor
		state.mu.Unlock()
		brush, _, _ := procCreateSolidBrush.Call(uintptr(color))
		r := rect{Left: 0, Top: 0, Right: int32(screenWidth()), Bottom: int32(screenHeight())}
		procFillRect.Call(hdc, uintptr(unsafe.Pointer(&r)), brush)
		procDeleteObject.Call(brush)
		procEndPaint.Call(hwnd, uintptr(unsafe.Pointer(&ps)))
		return 0
	case wmTimer:
		if wParam == dynamicTimerID {
			advanceDynamicFrame(hwnd)
			return 0
		}
		ret, _, _ := procDefWindowProcW.Call(hwnd, message, wParam, lParam)
		return ret
	case wmAppInvalidate:
		invalidateWindowNow(hwnd)
		return 0
	case wmAppShow:
		procShowWindow.Call(hwnd, swShow)
		invalidateWindowNow(hwnd)
		return 0
	case wmAppHide:
		procShowWindow.Call(hwnd, swHide)
		return 0
	case wmAppFocus:
		procSetForegroundWindow.Call(hwnd)
		return 0
	case wmAppDynamicStart:
		procKillTimer.Call(hwnd, dynamicTimerID)
		interval := wParam
		if interval == 0 {
			interval = 16
		}
		procSetTimer.Call(hwnd, dynamicTimerID, interval, 0)
		invalidateWindowNow(hwnd)
		return 0
	case wmAppDynamicStop:
		procKillTimer.Call(hwnd, dynamicTimerID)
		invalidateWindowNow(hwnd)
		return 0
	case wmKeyDown:
		appendEvent(hidEvent{Type: "key_down", Code: uint32(wParam), Modifiers: modifierState(), Qpc: qpcNow(), UnixNano: time.Now().UnixNano()})
		return 0
	case wmKeyUp:
		appendEvent(hidEvent{Type: "key_up", Code: uint32(wParam), Modifiers: modifierState(), Qpc: qpcNow(), UnixNano: time.Now().UnixNano()})
		return 0
	case wmSysKeyDown:
		appendEvent(hidEvent{Type: "key_down", Code: uint32(wParam), Modifiers: modifierState(), Description: "syskey", Qpc: qpcNow(), UnixNano: time.Now().UnixNano()})
		return 0
	case wmSysKeyUp:
		appendEvent(hidEvent{Type: "key_up", Code: uint32(wParam), Modifiers: modifierState(), Description: "syskey", Qpc: qpcNow(), UnixNano: time.Now().UnixNano()})
		return 0
	case wmChar:
		appendEvent(hidEvent{Type: "char", Code: uint32(wParam), Char: string(rune(wParam)), Modifiers: modifierState(), Qpc: qpcNow(), UnixNano: time.Now().UnixNano()})
		return 0
	case wmMouseMove:
		x, y := mouseXY(lParam)
		appendEvent(hidEvent{Type: "mouse_move", X: x, Y: y, Qpc: qpcNow(), UnixNano: time.Now().UnixNano()})
		return 0
	case wmLButtonDown, wmLButtonUp, wmRButtonDown, wmRButtonUp:
		x, y := mouseXY(lParam)
		appendEvent(hidEvent{Type: mouseMessageName(msg), X: x, Y: y, Qpc: qpcNow(), UnixNano: time.Now().UnixNano()})
		return 0
	case wmMouseWheel:
		delta := int16((wParam >> 16) & 0xffff)
		appendEvent(hidEvent{Type: "mouse_wheel", WheelDelta: delta, Qpc: qpcNow(), UnixNano: time.Now().UnixNano()})
		return 0
	case wmClose:
		procShowWindow.Call(hwnd, swHide)
		return 0
	case wmDestroy:
		procKillTimer.Call(hwnd, dynamicTimerID)
		procPostQuitMessage.Call(0)
		return 0
	default:
		ret, _, _ := procDefWindowProcW.Call(hwnd, message, wParam, lParam)
		return ret
	}
}

func mouseMessageName(message uint32) string {
	switch message {
	case wmLButtonDown:
		return "mouse_left_down"
	case wmLButtonUp:
		return "mouse_left_up"
	case wmRButtonDown:
		return "mouse_right_down"
	case wmRButtonUp:
		return "mouse_right_up"
	default:
		return "mouse_button"
	}
}

func mouseXY(lParam uintptr) (int32, int32) {
	x := int16(lParam & 0xffff)
	y := int16((lParam >> 16) & 0xffff)
	return int32(x), int32(y)
}

func modifierState() uint32 {
	var mods uint32
	if isKeyDown(vkLControl) {
		mods |= modLeftCtrl
	}
	if isKeyDown(vkLShift) {
		mods |= modLeftShift
	}
	if isKeyDown(vkLMenu) {
		mods |= modLeftAlt
	}
	if isKeyDown(vkLWin) {
		mods |= modLeftMeta
	}
	if isKeyDown(vkRControl) {
		mods |= modRightCtrl
	}
	if isKeyDown(vkRShift) {
		mods |= modRightShift
	}
	if isKeyDown(vkRMenu) {
		mods |= modRightAlt
	}
	if isKeyDown(vkRWin) {
		mods |= modRightMeta
	}
	return mods
}

func isKeyDown(vk uintptr) bool {
	ret, _, _ := procGetKeyState.Call(vk)
	return int16(ret&0xffff) < 0
}

func appendEvent(e hidEvent) {
	state.mu.Lock()
	defer state.mu.Unlock()
	state.events = append(state.events, e)
	if len(state.events) > 2048 {
		state.events = append([]hidEvent(nil), state.events[len(state.events)-2048:]...)
	}
}

func clearEvents() {
	state.mu.Lock()
	state.events = nil
	state.mu.Unlock()
}

func snapshotEvents() []hidEvent {
	state.mu.Lock()
	defer state.mu.Unlock()
	out := make([]hidEvent, len(state.events))
	copy(out, state.events)
	return out
}

func setColor(value string) map[string]interface{} {
	stopDynamic()
	return applyColor(value)
}

func applyColor(value string) map[string]interface{} {
	display := updateColorState(value)
	postUIMessage(wmAppInvalidate, 0, 0)
	return display
}

func updateColorState(value string) map[string]interface{} {
	colorHex := normalizeColor(value)
	state.mu.Lock()
	display := setColorStateLocked(colorHex)
	state.mu.Unlock()
	return display
}

func startDynamic(fps int) map[string]interface{} {
	if fps < 1 {
		fps = 60
	}
	if fps > 120 {
		fps = 120
	}
	stopDynamic()
	state.mu.Lock()
	state.dynamicActive = true
	state.dynamicFPS = fps
	state.dynamicFrame = 0
	display := setColorStateLocked(dynamicFrameColor(0))
	state.mu.Unlock()
	postUIMessage(wmAppDynamicStart, uintptr(dynamicTimerIntervalMS(fps)), 0)
	display["dynamic"] = true
	display["fps"] = fps
	return display
}

func stopDynamic() {
	state.mu.Lock()
	active := state.dynamicActive
	state.dynamicActive = false
	state.dynamicFPS = 0
	state.dynamicFrame = 0
	state.mu.Unlock()
	if active {
		postUIMessage(wmAppDynamicStop, 0, 0)
	}
}

func dynamicTimerIntervalMS(fps int) int {
	if fps < 1 {
		fps = 60
	}
	interval := 1000 / fps
	if interval < 1 {
		return 1
	}
	return interval
}

func dynamicFrameColor(frame int64) string {
	r := uint8((frame*5 + 31) % 256)
	g := uint8((frame*13 + 97) % 256)
	b := uint8((frame*29 + 173) % 256)
	return fmt.Sprintf("#%02x%02x%02x", r, g, b)
}

func advanceDynamicFrame(hwnd uintptr) {
	state.mu.Lock()
	if !state.dynamicActive {
		state.mu.Unlock()
		return
	}
	state.dynamicFrame++
	setColorStateLocked(dynamicFrameColor(state.dynamicFrame))
	state.mu.Unlock()
	invalidateWindowNow(hwnd)
}

func setColorStateLocked(colorHex string) map[string]interface{} {
	color := parseColor(colorHex)
	qpc := qpcNow()
	unixNano := time.Now().UnixNano()
	state.bgColor = color
	state.colorHex = colorHex
	state.lastColorChangeQpc = qpc
	state.lastColorChangeUnix = unixNano
	state.colorSequence++
	seq := state.colorSequence
	return map[string]interface{}{
		"color":                 colorHex,
		"qpc":                   qpc,
		"unix_nano":             unixNano,
		"last_change_qpc":       qpc,
		"last_change_unix_nano": unixNano,
		"sequence":              seq,
	}
}

func currentDisplayState() map[string]interface{} {
	state.mu.Lock()
	defer state.mu.Unlock()
	return map[string]interface{}{
		"color":                 state.colorHex,
		"last_change_qpc":       state.lastColorChangeQpc,
		"last_change_unix_nano": state.lastColorChangeUnix,
		"sequence":              state.colorSequence,
		"dynamic":               state.dynamicActive,
		"dynamic_fps":           state.dynamicFPS,
		"qpc":                   qpcNow(),
		"unix_nano":             time.Now().UnixNano(),
	}
}

func showWindow(show bool) {
	state.mu.Lock()
	hwnd := state.hwnd
	state.mu.Unlock()
	if hwnd == 0 {
		return
	}
	if show {
		postUIMessage(wmAppShow, 0, 0)
	} else {
		postUIMessage(wmAppHide, 0, 0)
	}
}

func focusWindow() {
	state.mu.Lock()
	hwnd := state.hwnd
	state.mu.Unlock()
	if hwnd != 0 {
		postUIMessage(wmAppFocus, 0, 0)
	}
}

func postUIMessage(message uint32, wParam uintptr, lParam uintptr) {
	state.mu.Lock()
	hwnd := state.hwnd
	state.mu.Unlock()
	if hwnd != 0 {
		procPostMessageW.Call(hwnd, uintptr(message), wParam, lParam)
	}
}

func invalidateWindowNow(hwnd uintptr) {
	procInvalidateRect.Call(hwnd, 0, 1)
	procUpdateWindow.Call(hwnd)
}

func disableIME(hwnd uintptr) {
	if hwnd != 0 {
		procImmAssociateContext.Call(hwnd, 0)
	}
}

func parseColor(value string) uint32 {
	s := strings.TrimPrefix(strings.TrimSpace(value), "#")
	if len(s) != 6 {
		return 0x0000ff00
	}
	var rgb uint64
	_, err := fmt.Sscanf(s, "%06x", &rgb)
	if err != nil {
		return 0x0000ff00
	}
	r := rgb >> 16 & 0xff
	g := rgb >> 8 & 0xff
	b := rgb & 0xff
	return uint32(r | (g << 8) | (b << 16))
}

func normalizeColor(value string) string {
	s := strings.TrimPrefix(strings.TrimSpace(value), "#")
	if len(s) != 6 {
		return "#00ff00"
	}
	var rgb uint64
	if _, err := fmt.Sscanf(s, "%06x", &rgb); err != nil {
		return "#00ff00"
	}
	return fmt.Sprintf("#%06x", rgb)
}

func screenSize() (int, int) {
	return screenWidth(), screenHeight()
}

func screenInfo() map[string]int {
	width, height := screenSize()
	return map[string]int{"width": width, "height": height}
}

func screenWidth() int {
	r, _, _ := procGetSystemMetrics.Call(0)
	return int(r)
}

func screenHeight() int {
	r, _, _ := procGetSystemMetrics.Call(1)
	return int(r)
}

func hostname() string {
	h, err := os.Hostname()
	if err != nil {
		return "unknown"
	}
	return h
}

func getModuleHandle() uintptr {
	ret, _, _ := procGetModuleHandleW.Call(0)
	return ret
}

func qpcNow() int64 {
	var value int64
	procQueryPerformanceCount.Call(uintptr(unsafe.Pointer(&value)))
	return value
}

func qpcFreq() int64 {
	var value int64
	procQueryPerformanceFreq.Call(uintptr(unsafe.Pointer(&value)))
	return value
}

func listDrives() []driveInfo {
	mask, _, _ := procGetLogicalDrives.Call()
	now := time.Now().UnixNano()
	drives := []driveInfo{}
	for i := 0; i < 26; i++ {
		if mask&(1<<uint(i)) == 0 {
			continue
		}
		letter := string(rune('A' + i))
		root := letter + ":\\"
		rootPtr := syscall.StringToUTF16Ptr(root)
		typ, _, _ := procGetDriveTypeW.Call(uintptr(unsafe.Pointer(rootPtr)))
		info := driveInfo{
			Letter:       letter,
			Root:         root,
			Type:         uint32(typ),
			Removable:    uint32(typ) == driveRemovable,
			WriteCapable: true,
			ObservedNano: now,
		}
		info.Label, info.FileSystem = volumeInfo(root)
		info.TotalBytes, info.FreeBytes = diskSpace(root)
		drives = append(drives, info)
	}
	return drives
}

func volumeInfo(root string) (string, string) {
	rootPtr := syscall.StringToUTF16Ptr(root)
	label := make([]uint16, 260)
	fs := make([]uint16, 260)
	procGetVolumeInformationW.Call(
		uintptr(unsafe.Pointer(rootPtr)),
		uintptr(unsafe.Pointer(&label[0])),
		uintptr(len(label)),
		0,
		0,
		0,
		uintptr(unsafe.Pointer(&fs[0])),
		uintptr(len(fs)),
	)
	return syscall.UTF16ToString(label), syscall.UTF16ToString(fs)
}

func diskSpace(root string) (uint64, uint64) {
	rootPtr := syscall.StringToUTF16Ptr(root)
	var freeAvail, total, totalFree uint64
	procGetDiskFreeSpaceExW.Call(
		uintptr(unsafe.Pointer(rootPtr)),
		uintptr(unsafe.Pointer(&freeAvail)),
		uintptr(unsafe.Pointer(&total)),
		uintptr(unsafe.Pointer(&totalFree)),
	)
	return total, totalFree
}

func waitNewDrive(known []string, timeout time.Duration) (driveInfo, error) {
	knownSet := map[string]bool{}
	for _, root := range known {
		knownSet[strings.ToUpper(root)] = true
	}
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		for _, drive := range listDrives() {
			if !knownSet[strings.ToUpper(drive.Root)] &&
				(drive.Type == driveRemovable || drive.Type == driveFixed || drive.Type == driveUnknown || drive.Type == driveNoRootDir) {
				return drive, nil
			}
		}
		time.Sleep(500 * time.Millisecond)
	}
	return driveInfo{}, fmt.Errorf("timed out waiting for new drive")
}

func waitRemoved(root string, timeout time.Duration) error {
	target := strings.ToUpper(root)
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		found := false
		for _, drive := range listDrives() {
			if strings.ToUpper(drive.Root) == target {
				found = true
				break
			}
		}
		if !found {
			return nil
		}
		time.Sleep(500 * time.Millisecond)
	}
	return fmt.Errorf("timed out waiting for drive removal: %s", root)
}

func writeReadVerify(root, filename string, sizeBytes int) (map[string]interface{}, error) {
	if root == "" {
		return nil, fmt.Errorf("root is required")
	}
	if !strings.HasSuffix(root, "\\") {
		root += "\\"
	}
	path := filepath.Join(root, filename)
	data := make([]byte, sizeBytes)
	r := rand.New(rand.NewSource(0x0badc0de))
	if _, err := r.Read(data); err != nil {
		return nil, err
	}
	expected := sha256.Sum256(data)
	start := qpcNow()
	writeStartWall := time.Now()
	file, err := os.Create(path)
	if err != nil {
		return nil, err
	}
	written, err := file.Write(data)
	if err == nil && written != len(data) {
		err = fmt.Errorf("short write: %d/%d", written, len(data))
	}
	if err == nil {
		err = file.Sync()
	}
	closeErr := file.Close()
	if err != nil {
		_ = os.Remove(path)
		return nil, err
	}
	if closeErr != nil {
		_ = os.Remove(path)
		return nil, closeErr
	}
	writeDoneWall := time.Now()
	writtenAt := qpcNow()
	readMode := "uncached"
	readNote := ""
	readCached := false
	sectorSize := diskBytesPerSector(root)
	readStartWall := time.Now()
	readBack, err := readFileUncached(path, sizeBytes, sectorSize)
	if err != nil {
		readMode = "cached_fallback"
		readNote = err.Error()
		readCached = true
		readStartWall = time.Now()
		readBack, err = os.ReadFile(path)
		if err != nil {
			_ = os.Remove(path)
			return nil, err
		}
	}
	readDoneWall := time.Now()
	readAt := qpcNow()
	actual := sha256.Sum256(readBack)
	if expected != actual {
		_ = os.Remove(path)
		return nil, fmt.Errorf("sha256 mismatch after readback")
	}
	_ = os.Remove(path)
	writeMS := elapsedMS(writeStartWall, writeDoneWall)
	readMS := elapsedMS(readStartWall, readDoneWall)
	totalMS := elapsedMS(writeStartWall, readDoneWall)
	result := map[string]interface{}{
		"path":          path,
		"size_bytes":    sizeBytes,
		"write_bytes":   written,
		"read_bytes":    len(readBack),
		"sha256":        hex.EncodeToString(actual[:]),
		"write_ms":      writeMS,
		"read_ms":       readMS,
		"total_ms":      totalMS,
		"write_mib_s":   throughputMiBS(written, writeMS),
		"read_mode":     readMode,
		"read_cached":   readCached,
		"sector_size":   sectorSize,
		"qpc_start":     start,
		"qpc_written":   writtenAt,
		"qpc_read_done": readAt,
		"unix_nano":     time.Now().UnixNano(),
	}
	readSpeed := throughputMiBS(len(readBack), readMS)
	if readCached {
		result["cached_read_mib_s"] = readSpeed
		result["read_note"] = readNote
	} else {
		result["read_mib_s"] = readSpeed
	}
	return result, nil
}

func elapsedMS(start, end time.Time) float64 {
	return float64(end.Sub(start).Microseconds()) / 1000
}

func throughputMiBS(bytes int, ms float64) float64 {
	if bytes <= 0 || ms <= 0 {
		return 0
	}
	return (float64(bytes) / 1024 / 1024) / (ms / 1000)
}

func diskBytesPerSector(root string) int {
	if root == "" {
		return 4096
	}
	if !strings.HasSuffix(root, "\\") {
		root += "\\"
	}
	rootPtr := syscall.StringToUTF16Ptr(root)
	var sectorsPerCluster, bytesPerSector, freeClusters, totalClusters uint32
	ret, _, _ := procGetDiskFreeSpaceW.Call(
		uintptr(unsafe.Pointer(rootPtr)),
		uintptr(unsafe.Pointer(&sectorsPerCluster)),
		uintptr(unsafe.Pointer(&bytesPerSector)),
		uintptr(unsafe.Pointer(&freeClusters)),
		uintptr(unsafe.Pointer(&totalClusters)),
	)
	if ret == 0 || bytesPerSector == 0 {
		return 4096
	}
	if bytesPerSector < 4096 {
		return 4096
	}
	return int(bytesPerSector)
}

func readFileUncached(path string, sizeBytes int, alignment int) ([]byte, error) {
	if sizeBytes <= 0 {
		return []byte{}, nil
	}
	if alignment < 512 {
		alignment = 4096
	}
	if sizeBytes%alignment != 0 {
		return nil, fmt.Errorf("uncached read size %d is not aligned to %d bytes", sizeBytes, alignment)
	}
	pathPtr := syscall.StringToUTF16Ptr(path)
	handle, _, callErr := procCreateFileW.Call(
		uintptr(unsafe.Pointer(pathPtr)),
		genericRead,
		fileShareRead|fileShareWrite,
		0,
		openExisting,
		fileAttributeNormal|fileFlagNoBuffering|fileFlagSequentialScan,
		0,
	)
	if handle == ^uintptr(0) {
		return nil, winCallError("CreateFileW FILE_FLAG_NO_BUFFERING", callErr)
	}
	defer procCloseHandle.Call(handle)

	raw := make([]byte, sizeBytes+alignment)
	base := uintptr(unsafe.Pointer(&raw[0]))
	aligned := (base + uintptr(alignment-1)) &^ uintptr(alignment-1)
	buf := unsafe.Slice((*byte)(unsafe.Pointer(aligned)), sizeBytes)
	total := 0
	for total < sizeBytes {
		chunk := sizeBytes - total
		if chunk > 1024*1024 {
			chunk = 1024 * 1024
		}
		chunk -= chunk % alignment
		if chunk <= 0 {
			return nil, fmt.Errorf("invalid uncached read chunk for alignment %d", alignment)
		}
		var read uint32
		ret, _, err := procReadFile.Call(
			handle,
			uintptr(unsafe.Pointer(&buf[total])),
			uintptr(chunk),
			uintptr(unsafe.Pointer(&read)),
			0,
		)
		if ret == 0 {
			return nil, winCallError("ReadFile FILE_FLAG_NO_BUFFERING", err)
		}
		if read == 0 {
			break
		}
		total += int(read)
	}
	if total != sizeBytes {
		return nil, fmt.Errorf("short uncached read: %d/%d", total, sizeBytes)
	}
	out := make([]byte, sizeBytes)
	copy(out, buf)
	return out, nil
}

func winCallError(operation string, err error) error {
	if errno, ok := err.(syscall.Errno); ok && errno == 0 {
		return fmt.Errorf("%s failed", operation)
	}
	return fmt.Errorf("%s failed: %w", operation, err)
}
