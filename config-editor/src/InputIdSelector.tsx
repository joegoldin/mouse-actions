import {
  Buttons,
  ButtonType,
  InputIdType,
  InputKinds,
  Keys,
  KeyType,
} from "./config.type";
import { MenuItem, Select } from "@mui/material";
import { startCase } from "lodash";

/**
 * Single dropdown for "either a mouse button or a key" — the kind selector
 * sits next to the code selector. Used by modifier_remap rules where
 * while_held / trigger / emit each can be a mouse button or a keyboard key.
 */
export function InputIdSelector({
  value,
  onChange,
  label,
}: {
  value: InputIdType;
  onChange: (next: InputIdType) => void;
  label?: string;
}) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
      {label && (
        <div style={{ fontSize: 12, color: "#666" }}>{label}</div>
      )}
      <div style={{ display: "flex", gap: 6 }}>
        <Select
          size="small"
          value={value.kind}
          onChange={(e) => {
            const kind = e.target.value as InputIdType["kind"];
            if (kind === "Mouse") {
              onChange({ kind: "Mouse", code: "Right" });
            } else {
              onChange({ kind: "Key", code: "ShiftLeft" });
            }
          }}
        >
          {InputKinds.map((k) => (
            <MenuItem key={k} value={k}>
              {k}
            </MenuItem>
          ))}
        </Select>
        {value.kind === "Mouse" ? (
          <Select
            size="small"
            value={value.code}
            style={{ minWidth: 120 }}
            onChange={(e) =>
              onChange({ kind: "Mouse", code: e.target.value as ButtonType })
            }
          >
            {Buttons.map((b) => (
              <MenuItem key={b} value={b}>
                {startCase(b)}
              </MenuItem>
            ))}
          </Select>
        ) : (
          <Select
            size="small"
            value={value.code}
            style={{ minWidth: 140 }}
            onChange={(e) =>
              onChange({ kind: "Key", code: e.target.value as KeyType })
            }
          >
            {Keys.map((k) => (
              <MenuItem key={k} value={k}>
                {k}
              </MenuItem>
            ))}
          </Select>
        )}
      </div>
    </div>
  );
}
