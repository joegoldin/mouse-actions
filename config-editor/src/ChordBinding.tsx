import {
  Checkbox,
  FormControlLabel,
  IconButton,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
} from "@mui/material";
import AddIcon from "@mui/icons-material/Add";
import DeleteIcon from "@mui/icons-material/Delete";
import { memo } from "react";
import { isEqual, startCase } from "lodash";

import {
  Buttons,
  ButtonType,
  ChordBindingType,
} from "./config.type";

/**
 * Editor for one ChordBinding: a set of mouse buttons that, when pressed
 * within a window, trigger a shell command.
 */
export function ChordBinding({
  chord,
  setChord,
  addChord,
  deleteChord,
}: {
  chord: ChordBindingType;
  setChord: (next: ChordBindingType) => void;
  addChord: (after: ChordBindingType) => void;
  deleteChord: (current: ChordBindingType) => void;
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
          title="Delete this chord binding"
          color="warning"
          onClick={() => deleteChord(chord)}
        >
          <DeleteIcon />
        </IconButton>
        <IconButton
          title="Add a chord binding below"
          color="primary"
          onClick={() => addChord(chord)}
        >
          <AddIcon />
        </IconButton>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        <TextField
          size="small"
          label="Comment"
          variant="outlined"
          value={chord.comment ?? ""}
          onChange={(e) =>
            setChord(structuredClone({ ...chord, comment: e.target.value }))
          }
        />
        <TextField
          size="small"
          label="Command"
          variant="outlined"
          value={chord.cmd_str}
          onChange={(e) =>
            setChord(structuredClone({ ...chord, cmd_str: e.target.value }))
          }
        />
        <div>
          <div style={{ fontSize: 12, color: "#666", marginBottom: 4 }}>
            Buttons (pressed within window)
          </div>
          <ToggleButtonGroup
            size="small"
            value={chord.buttons}
            onChange={(_, next) =>
              setChord(
                structuredClone({
                  ...chord,
                  buttons: (next as ButtonType[]) ?? [],
                })
              )
            }
            color="primary"
            style={{ flexWrap: "wrap" }}
          >
            {Buttons.map((b) => (
              <ToggleButton key={b} value={b}>
                {startCase(b)}
              </ToggleButton>
            ))}
          </ToggleButtonGroup>
        </div>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        <TextField
          size="small"
          label="Window (ms)"
          type="number"
          variant="outlined"
          value={chord.window_ms ?? 100}
          inputProps={{ min: 1 }}
          onChange={(e) =>
            setChord(
              structuredClone({
                ...chord,
                window_ms: parseInt(e.target.value, 10) || 0,
              })
            )
          }
        />
        <FormControlLabel
          control={
            <Checkbox
              checked={chord.passthrough ?? true}
              onChange={(e) =>
                setChord(
                  structuredClone({
                    ...chord,
                    passthrough: e.target.checked,
                  })
                )
              }
            />
          }
          label="Pass-through original button events"
        />
      </div>
    </div>
  );
}

export const ChordBindingMemo = memo(ChordBinding, (prev, next) =>
  isEqual(prev.chord, next.chord)
);
