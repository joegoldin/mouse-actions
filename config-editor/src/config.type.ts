export const Buttons = [
  "Left",
  "Right",
  "Middle",
  "Side",
  "Extra",
  "Forward",
  "Back",
  "Task",
  "Trigger",
  "Thumb",
  "Thumb2",
  "WheelUp",
  "WheelDown",
  "Unknown",
  "None",
] as const;
export type ButtonType = (typeof Buttons)[number];

export const Modifiers = [
  "ShiftLeft",
  "ShiftRight",
  "ControlLeft",
  "ControlRight",
  "MetaLeft",
  "Alt",
  "AltGr",
] as const;

export type ModifierType = (typeof Modifiers)[number];

export type Point = { x: number; y: number };

export const EventTypes = ["Press", "Release", "Click", "Shape"] as const;
export type EventTypeType = (typeof EventTypes)[number];

export const Edges = ["Top", "Right", "Bottom", "Left"] as const;
export type EdgeType = (typeof Edges)[number];

export type EventType = {
  button: ButtonType;
  modifiers?: ModifierType[];
  event_type: EventTypeType;
  edges?: EdgeType[];
  shapes_xy?: number[][];
};

export type BindingType = {
  uid?: string;
  comment: string;
  cmd_str: string;
  event: EventType;
};

// ---------------------------------------------------------------------------
// Generic inputs (mouse button OR keyboard key) — used by modifier remaps.
// ---------------------------------------------------------------------------

export const Keys = [
  "Alt",
  "AltGr",
  "ShiftLeft",
  "ShiftRight",
  "ControlLeft",
  "ControlRight",
  "MetaLeft",
  "MetaRight",
  "CapsLock",
  "NumLock",
  "ScrollLock",
  "Escape",
  "Tab",
  "Backspace",
  "Return",
  "Space",
  "Insert",
  "Delete",
  "Home",
  "End",
  "PageUp",
  "PageDown",
  "UpArrow",
  "DownArrow",
  "LeftArrow",
  "RightArrow",
  "Pause",
  "PrintScreen",
  "F1",
  "F2",
  "F3",
  "F4",
  "F5",
  "F6",
  "F7",
  "F8",
  "F9",
  "F10",
  "F11",
  "F12",
  "KeyA",
  "KeyB",
  "KeyC",
  "KeyD",
  "KeyE",
  "KeyF",
  "KeyG",
  "KeyH",
  "KeyI",
  "KeyJ",
  "KeyK",
  "KeyL",
  "KeyM",
  "KeyN",
  "KeyO",
  "KeyP",
  "KeyQ",
  "KeyR",
  "KeyS",
  "KeyT",
  "KeyU",
  "KeyV",
  "KeyW",
  "KeyX",
  "KeyY",
  "KeyZ",
  "Num0",
  "Num1",
  "Num2",
  "Num3",
  "Num4",
  "Num5",
  "Num6",
  "Num7",
  "Num8",
  "Num9",
  "Minus",
  "Equal",
  "LeftBracket",
  "RightBracket",
  "SemiColon",
  "Quote",
  "BackQuote",
  "BackSlash",
  "IntlBackslash",
  "Comma",
  "Dot",
  "Slash",
] as const;
export type KeyType = (typeof Keys)[number];

export const InputKinds = ["Mouse", "Key"] as const;
export type InputKindType = (typeof InputKinds)[number];

export type InputIdType =
  | { kind: "Mouse"; code: ButtonType }
  | { kind: "Key"; code: KeyType };

export const RemapModes = ["Hold", "Toggle"] as const;
export type RemapModeType = (typeof RemapModes)[number];

export type ModifierRemapType = {
  uid?: string;
  comment?: string;
  while_held: InputIdType;
  trigger: InputIdType;
  emit: InputIdType;
  mode?: RemapModeType;
  release_delay_ms?: number;
};

export type ChordBindingType = {
  uid?: string;
  comment?: string;
  buttons: ButtonType[];
  window_ms?: number;
  cmd_str: string;
  passthrough?: boolean;
};

export type ConfigType = {
  shape_button: ButtonType;
  bindings: BindingType[];
  modifier_remaps?: ModifierRemapType[];
  chord_bindings?: ChordBindingType[];
};
