import {
  IconButton,
  MenuItem,
  Select,
  TextField,
  Typography,
} from "@mui/material";
import AddIcon from "@mui/icons-material/Add";
import DeleteIcon from "@mui/icons-material/Delete";
import { memo } from "react";
import { isEqual } from "lodash";

import {
  ModifierRemapType,
  RemapModes,
  RemapModeType,
} from "./config.type";
import { InputIdSelector } from "./InputIdSelector";

/**
 * Editor for one ModifierRemap rule.
 * Shape: "while {while_held} is held, when {trigger} is pressed, emit {emit}"
 */
export function ModifierRemap({
  remap,
  setRemap,
  addRemap,
  deleteRemap,
}: {
  remap: ModifierRemapType;
  setRemap: (next: ModifierRemapType) => void;
  addRemap: (after: ModifierRemapType) => void;
  deleteRemap: (current: ModifierRemapType) => void;
}) {
  return (
    <div
      style={{
        marginBottom: 16,
        padding: 10,
        maxWidth: 1000,
        display: "grid",
        gridTemplateColumns: "30px 1fr 170px",
        gap: 10,
        borderBottom: "solid #aaa 2px",
      }}
    >
      <div style={{ display: "flex", flexDirection: "column" }}>
        <IconButton
          title="Delete this modifier remap"
          color="warning"
          onClick={() => deleteRemap(remap)}
        >
          <DeleteIcon />
        </IconButton>
        <IconButton
          title="Add a modifier remap below"
          color="primary"
          onClick={() => addRemap(remap)}
        >
          <AddIcon />
        </IconButton>
      </div>
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          gap: 10,
        }}
      >
        <TextField
          size="small"
          label="Comment"
          variant="outlined"
          value={remap.comment ?? ""}
          onChange={(e) =>
            setRemap(structuredClone({ ...remap, comment: e.target.value }))
          }
        />
        <div style={{ display: "flex", flexWrap: "wrap", gap: 16, alignItems: "flex-end" }}>
          <InputIdSelector
            label="While held"
            value={remap.while_held}
            onChange={(next) =>
              setRemap(structuredClone({ ...remap, while_held: next }))
            }
          />
          <Typography style={{ alignSelf: "center", color: "#666" }}>
            +
          </Typography>
          <InputIdSelector
            label="Trigger"
            value={remap.trigger}
            onChange={(next) =>
              setRemap(structuredClone({ ...remap, trigger: next }))
            }
          />
          <Typography style={{ alignSelf: "center", color: "#666" }}>
            →
          </Typography>
          <InputIdSelector
            label="Emit"
            value={remap.emit}
            onChange={(next) =>
              setRemap(structuredClone({ ...remap, emit: next }))
            }
          />
        </div>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        <div>
          <div style={{ fontSize: 12, color: "#666", marginBottom: 4 }}>
            Mode
          </div>
          <Select
            size="small"
            value={remap.mode ?? "Toggle"}
            style={{ width: "100%" }}
            onChange={(e) =>
              setRemap(
                structuredClone({
                  ...remap,
                  mode: e.target.value as RemapModeType,
                })
              )
            }
          >
            {RemapModes.map((m) => (
              <MenuItem key={m} value={m}>
                {m}
              </MenuItem>
            ))}
          </Select>
        </div>
        <TextField
          size="small"
          label="Release delay (ms)"
          type="number"
          variant="outlined"
          value={remap.release_delay_ms ?? 25}
          inputProps={{ min: 0 }}
          onChange={(e) =>
            setRemap(
              structuredClone({
                ...remap,
                release_delay_ms: parseInt(e.target.value, 10) || 0,
              })
            )
          }
        />
      </div>
    </div>
  );
}

export const ModifierRemapMemo = memo(ModifierRemap, (prev, next) =>
  isEqual(prev.remap, next.remap)
);
