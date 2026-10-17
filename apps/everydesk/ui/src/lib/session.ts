// Session API for the minimal controller: Tauri commands/events plus the pure helpers
// (frame header parsing, mouse masks, coordinate mapping, key names) used by RemoteCanvas.
// The pure helpers carry no DOM/Tauri dependency so `node --test` can exercise them.

import { Channel, invoke } from '@tauri-apps/api/core'
import { writable } from 'svelte/store'

export interface DisplayRect {
  x: number
  y: number
  w: number
  h: number
}

export interface MsgBox {
  kind: string
  title: string
  text: string
}

export interface Quality {
  fps: number | null
  delay: number | null
  speed: string | null
}

export const NO_QUALITY: Quality = { fps: null, delay: null, speed: null }

/** A `session:quality` payload may carry only some fields: keep the last known value of the rest. */
export function mergeQuality(prev: Quality, q: Partial<Quality>): Quality {
  return { fps: q.fps ?? prev.fps, delay: q.delay ?? prev.delay, speed: q.speed ?? prev.speed }
}

// ---- live session state (fed by RemoteCanvas, read by the pill) ---------------------

/** Latest `session:quality` reading (fps, delay in ms, speed). */
export const quality = writable<Quality>(NO_QUALITY)
/** The peer's displays (`session:displays`). */
export const displayList = writable<DisplayRect[]>([])
/** Index of the display the frames currently come from. */
export const currentDisplay = writable(0)

export function resetSessionState() {
  quality.set(NO_QUALITY)
  displayList.set([])
  currentDisplay.set(0)
}

export interface Modifiers {
  alt: boolean
  ctrl: boolean
  shift: boolean
  command: boolean
}

// ---- frame packets ------------------------------------------------------------------

export const FRAME_HEADER_LEN = 16

export interface FrameHeader {
  display: number
  width: number
  height: number
  stride: number
}

/** 16-byte little-endian header: display u32, width u32, height u32, stride u32. */
export function parseFrameHeader(buf: ArrayBuffer): FrameHeader | null {
  if (buf.byteLength < FRAME_HEADER_LEN) return null
  const v = new DataView(buf, 0, FRAME_HEADER_LEN)
  const h = {
    display: v.getUint32(0, true),
    width: v.getUint32(4, true),
    height: v.getUint32(8, true),
    stride: v.getUint32(12, true),
  }
  if (h.stride < h.width * 4 || buf.byteLength < FRAME_HEADER_LEN + h.stride * h.height) return null
  return h
}

// ---- mouse --------------------------------------------------------------------------

/** RustDesk `MOUSE_TYPE_*`. */
export const MouseEvt = { move: 0, down: 1, up: 2, wheel: 3 } as const
/** RustDesk `MOUSE_BUTTON_*`. */
export const MouseButton = { none: 0, left: 1, right: 2, wheel: 4 } as const

/** `mask = (button << 3) | event`, mirrors `commands::mouse_mask` in Rust. */
export function mouseMask(button: number, event: number): number {
  return (button << 3) | event
}

/** DOM `MouseEvent.button` (0 left, 1 middle, 2 right) → RustDesk button. */
export function domButton(button: number): number {
  switch (button) {
    case 0:
      return MouseButton.left
    case 1:
      return MouseButton.wheel
    case 2:
      return MouseButton.right
    default:
      return MouseButton.none
  }
}

export interface Box {
  left: number
  top: number
  width: number
  height: number
}

/**
 * Maps a client (viewport) point over a canvas drawn with `object-fit: contain` to remote
 * screen coordinates. `frameW/H` is the canvas' intrinsic size (the decoded frame);
 * `display` is that display's rect in the peer's virtual screen (origin + size, which may
 * differ from the frame size when the peer scales). Returns null outside the picture.
 */
export function clientToDisplay(
  clientX: number,
  clientY: number,
  box: Box,
  frameW: number,
  frameH: number,
  display: DisplayRect | undefined,
): { x: number; y: number } | null {
  if (frameW <= 0 || frameH <= 0 || box.width <= 0 || box.height <= 0) return null
  const scale = Math.min(box.width / frameW, box.height / frameH)
  const left = box.left + (box.width - frameW * scale) / 2
  const top = box.top + (box.height - frameH * scale) / 2
  const fx = (clientX - left) / scale
  const fy = (clientY - top) / scale
  if (fx < 0 || fy < 0 || fx >= frameW || fy >= frameH) return null
  const d = display ?? { x: 0, y: 0, w: frameW, h: frameH }
  const sx = d.w > 0 ? d.w / frameW : 1
  const sy = d.h > 0 ? d.h / frameH : 1
  return { x: d.x + Math.floor(fx * sx), y: d.y + Math.floor(fy * sy) }
}

/** Browser wheel delta → one RustDesk wheel step (same sign convention as Flutter). */
export function wheelSteps(deltaX: number, deltaY: number): { x: number; y: number } {
  if (Math.abs(deltaX) > Math.abs(deltaY)) return { x: -Math.sign(deltaX), y: 0 }
  return { x: 0, y: -Math.sign(deltaY) }
}

/** Pixels of wheel/trackpad travel per RustDesk wheel step. */
export const WHEEL_STEP_PX = 100
const LINE_PX = 34 // browsers report ~3 lines per notch in line mode (3 * 34 ≥ 100)

export interface WheelAcc {
  x: number
  y: number
}

/**
 * Accumulates wheel deltas (a Mac trackpad fires dozens of small events per gesture) and
 * returns how many whole steps to send per axis, signed like `wheelSteps` (Flutter's
 * convention: scrolling down → negative). The remainder stays in `acc`; reversing
 * direction drops the opposite remainder so a flick back responds at once.
 */
export function accumulateWheel(
  acc: WheelAcc,
  deltaX: number,
  deltaY: number,
  deltaMode = 0,
): { x: number; y: number } {
  const k = deltaMode === 1 ? LINE_PX : deltaMode === 2 ? WHEEL_STEP_PX : 1
  const take = (prev: number, d: number): [number, number] => {
    let v = Math.sign(d) !== 0 && Math.sign(d) !== Math.sign(prev) ? d * k : prev + d * k
    const n = Math.trunc(v / WHEEL_STEP_PX)
    v -= n * WHEEL_STEP_PX
    return [n === 0 ? 0 : -n, v]
  }
  const [sx, ax] = take(acc.x, deltaX)
  const [sy, ay] = take(acc.y, deltaY)
  acc.x = ax
  acc.y = ay
  return { x: sx, y: sy }
}

// ---- keyboard -----------------------------------------------------------------------

// `KeyboardEvent.code` → name understood by `Session::input_key` (`KEY_MAP` in
// src/client.rs, legacy keyboard mode). Letters/digits go as VK_A.. so the peer applies
// Shift itself; unmapped printable keys fall back to the character (`keyName`).
const CODE_TO_NAME: Record<string, string> = {
  Enter: 'VK_RETURN',
  NumpadEnter: 'VK_RETURN',
  Backspace: 'VK_BACK',
  Tab: 'VK_TAB',
  Escape: 'VK_ESCAPE',
  Space: 'VK_SPACE',
  ArrowLeft: 'VK_LEFT',
  ArrowUp: 'VK_UP',
  ArrowRight: 'VK_RIGHT',
  ArrowDown: 'VK_DOWN',
  Home: 'VK_HOME',
  End: 'VK_END',
  PageUp: 'VK_PRIOR',
  PageDown: 'VK_NEXT',
  Insert: 'VK_INSERT',
  Delete: 'VK_DELETE',
  CapsLock: 'VK_CAPITAL',
  ShiftLeft: 'VK_SHIFT',
  ShiftRight: 'RShift',
  ControlLeft: 'VK_CONTROL',
  ControlRight: 'RControl',
  AltLeft: 'VK_MENU',
  AltRight: 'RAlt',
  MetaLeft: 'Meta',
  MetaRight: 'RWin',
  ContextMenu: 'Apps',
  PrintScreen: 'VK_SNAPSHOT',
  ScrollLock: 'VK_SCROLL',
  Pause: 'VK_PAUSE',
  Comma: 'VK_COMMA',
  Slash: 'VK_SLASH',
  Semicolon: 'VK_SEMICOLON',
  Quote: 'VK_QUOTE',
  BracketLeft: 'VK_LBRACKET',
  BracketRight: 'VK_RBRACKET',
  Backslash: 'VK_BACKSLASH',
  Minus: 'VK_MINUS',
  Equal: 'VK_PLUS', // KEY_MAP: "it is =, but sciter return VK_PLUS"
  NumpadDivide: 'VK_DIVIDE',
  NumpadMultiply: 'VK_MULTIPLY',
  NumpadSubtract: 'VK_SUBTRACT',
  NumpadAdd: 'VK_ADD',
  NumpadDecimal: 'VK_DECIMAL',
}

/** Name for `session_key`, or null when the key cannot be sent. */
export function keyName(code: string, key: string): string | null {
  const mapped = CODE_TO_NAME[code]
  if (mapped) return mapped
  let m = /^Key([A-Z])$/.exec(code)
  if (m) return `VK_${m[1]}`
  m = /^Digit([0-9])$/.exec(code)
  if (m) return `VK_${m[1]}`
  m = /^Numpad([0-9])$/.exec(code)
  if (m) return `VK_NUMPAD${m[1]}`
  m = /^F([1-9]|1[0-2])$/.exec(code)
  if (m) return `VK_F${m[1]}`
  // Period, Backquote, IntlBackslash, ABNT keys...: send the produced character.
  if ([...key].length === 1) return key
  return null
}

// ---- Tauri bridge -------------------------------------------------------------------

export function connect(
  id: string,
  password: string,
  onFrame: (buf: ArrayBuffer) => void,
): Promise<void> {
  const frames = new Channel<ArrayBuffer>()
  frames.onmessage = onFrame
  return invoke('session_connect', { id, password, frames })
}

export const close = () => invoke('session_close')
export const login = (password: string) => invoke('session_login', { password })
export const frameAck = () => invoke('session_frame_ack')
export const switchDisplay = (index: number) => invoke('session_switch_display', { index })

export function sendMouse(mask: number, x: number, y: number, m: Modifiers): Promise<void> {
  return invoke('session_mouse', { mask, x, y, ...m })
}

export function sendKey(name: string, down: boolean, press: boolean, m: Modifiers) {
  return invoke('session_key', { name, down, press, ...m })
}

export const EVENTS = {
  displays: 'session:displays',
  connected: 'session:connected',
  msgbox: 'session:msgbox',
  closed: 'session:closed',
  quality: 'session:quality',
} as const

/** What a person types after hearing a code, as the backend's `memorable::normalize`: trim,
 *  lowercase, accents dropped (NFD, combining marks removed), and any run of spaces,
 *  underscores or dashes becomes one `-` ("Água Lua 42" → "agua-lua-42"). Numeric ids pass. */
export function normalizeCode(s: string): string {
  return s
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .split(/[\s_-]+/)
    .filter((p) => p.length > 0)
    .map((p) => p.toLowerCase())
    .join('-')
}
