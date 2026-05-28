import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import "./App.css";
import { BindingMemo } from "./Binding";
import {
  BindingType,
  ButtonType,
  ChordBindingType,
  ConfigType,
  ModifierRemapType,
} from "./config.type";
import { ButtonSelector } from "./ButtonSelector";
import { ModifierRemapMemo } from "./ModifierRemap";
import { ChordBindingMemo } from "./ChordBinding";
import { Button, ButtonGroup, Divider, Typography } from "@mui/material";
import PlayArrowIcon from "@mui/icons-material/PlayArrow";
import StopIcon from "@mui/icons-material/Stop";
import SaveIcon from "@mui/icons-material/Save";
import UndoIcon from "@mui/icons-material/Undo";
import GestureIcon from "@mui/icons-material/Gesture";
import AddIcon from "@mui/icons-material/Add";
import { AppSkeleton } from "./AppSkeleton";

export default function App() {
  const [isLoading, setIsLoading] = useState(false);
  const [defaultConfigPath, setGreetMsg] = useState("");
  const [version, setVersion] = useState("");
  const [config, setConfig] = useState<ConfigType>();
  const [shapeRecording, setShapeRecording] = useState(false);

  async function getDefaultConfigPath() {
    // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
    setGreetMsg(await invoke("get_default_config_path"));
  }

  useEffect(() => {
    invoke("get_version").then((v: any) => setVersion(v));
  }, []);

  // const [coords, setCoords] = useState<number[]>([
  //   0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1000, 1000,
  // ]);
  // const newCoords = useCoords(shapeRecording);
  // useEffect(() => {
  //   if (newCoords?.length) {
  //     setCoords(newCoords);
  //   }
  // }, [setCoords, newCoords]);

  const refreshConfig = async () => {
    setIsLoading(true);
    setTimeout(async () => {
      const newVconfig: ConfigType = await invoke("get_config");
      newVconfig.bindings.forEach((b) => (b.uid = self.crypto.randomUUID()));
      (newVconfig.modifier_remaps ?? []).forEach(
        (m) => (m.uid = self.crypto.randomUUID())
      );
      (newVconfig.chord_bindings ?? []).forEach(
        (c) => (c.uid = self.crypto.randomUUID())
      );
      setConfig(newVconfig);
      setIsLoading(false);
    }, 100);
  };
  useEffect(() => {
    refreshConfig();
  }, []);

  const onNewBinding = useCallback(
    (newBinding: BindingType) => {
      setConfig((prevConfig) => {
        if (prevConfig) {
          const newConfig = {
            ...prevConfig,
            binding: [...prevConfig?.bindings],
          };
          const index = prevConfig?.bindings.findIndex(
            (b) => b.uid === newBinding.uid
          );
          if (index >= 0) {
            newConfig.bindings[index] = newBinding;
            setConfig(newConfig);
          }
          return newConfig;
        } else {
          return prevConfig;
        }
      });
    },
    [setConfig]
  );

  const deleteBinding = useCallback(
    (binding: BindingType) => {
      setConfig((prevConfig) => {
        const index = prevConfig?.bindings.findIndex(
          (b) => b.uid === binding.uid
        );
        const newConfig: ConfigType = {
          shape_button: prevConfig?.shape_button || "Right",
          bindings: [...(prevConfig?.bindings || [])],
        };
        if (index !== undefined) {
          newConfig.bindings.splice(index, 1);
        }
        return newConfig;
      });
    },
    [setConfig]
  );

  const addBinding = useCallback(
    (binding?: BindingType) => {
      setConfig((prevConfig) => {
        const index = prevConfig?.bindings.findIndex(
          (b) => b.uid === binding?.uid
        );

        const newConfig: ConfigType = {
          shape_button: prevConfig?.shape_button || "Right",
          bindings: [...(prevConfig?.bindings || [])],
        };
        newConfig.bindings?.splice((index ?? -1) + 1, 0, {
          uid: self.crypto.randomUUID(),
          cmd_str: "TODO",
          comment: "TODO",
          event: {
            button: "Right",
            event_type: "Click",
            edges: [],
            modifiers: ["ControlLeft"],
            shapes_xy: [],
          },
        });
        return newConfig;
      });
    },
    [setConfig]
  );

  const setShapeButton = (shape_button: ButtonType) => {
    setConfig((prevConfig) => ({
      ...(prevConfig as ConfigType),
      bindings: [...(prevConfig?.bindings || [])],
      shape_button,
    }));
  };

  // --- modifier_remaps ----------------------------------------------------

  const setModifierRemap = useCallback((next: ModifierRemapType) => {
    setConfig((prev) => {
      if (!prev) return prev;
      const list = [...(prev.modifier_remaps ?? [])];
      const idx = list.findIndex((m) => m.uid === next.uid);
      if (idx >= 0) list[idx] = next;
      return { ...prev, modifier_remaps: list };
    });
  }, []);

  const deleteModifierRemap = useCallback((m: ModifierRemapType) => {
    setConfig((prev) => {
      if (!prev) return prev;
      return {
        ...prev,
        modifier_remaps: (prev.modifier_remaps ?? []).filter(
          (r) => r.uid !== m.uid
        ),
      };
    });
  }, []);

  const addModifierRemap = useCallback((after?: ModifierRemapType) => {
    setConfig((prev) => {
      if (!prev) return prev;
      const list = [...(prev.modifier_remaps ?? [])];
      const idx = after ? list.findIndex((m) => m.uid === after.uid) : -1;
      list.splice(idx + 1, 0, {
        uid: self.crypto.randomUUID(),
        comment: "",
        while_held: { kind: "Mouse", code: "Left" },
        trigger: { kind: "Mouse", code: "Right" },
        emit: { kind: "Key", code: "ShiftLeft" },
        mode: "Toggle",
        release_delay_ms: 25,
      });
      return { ...prev, modifier_remaps: list };
    });
  }, []);

  // --- chord_bindings -----------------------------------------------------

  const setChordBinding = useCallback((next: ChordBindingType) => {
    setConfig((prev) => {
      if (!prev) return prev;
      const list = [...(prev.chord_bindings ?? [])];
      const idx = list.findIndex((c) => c.uid === next.uid);
      if (idx >= 0) list[idx] = next;
      return { ...prev, chord_bindings: list };
    });
  }, []);

  const deleteChordBinding = useCallback((c: ChordBindingType) => {
    setConfig((prev) => {
      if (!prev) return prev;
      return {
        ...prev,
        chord_bindings: (prev.chord_bindings ?? []).filter(
          (x) => x.uid !== c.uid
        ),
      };
    });
  }, []);

  const addChordBinding = useCallback((after?: ChordBindingType) => {
    setConfig((prev) => {
      if (!prev) return prev;
      const list = [...(prev.chord_bindings ?? [])];
      const idx = after ? list.findIndex((c) => c.uid === after.uid) : -1;
      list.splice(idx + 1, 0, {
        uid: self.crypto.randomUUID(),
        comment: "",
        buttons: ["Side", "Extra"],
        window_ms: 100,
        cmd_str: "",
        passthrough: true,
      });
      return { ...prev, chord_bindings: list };
    });
  }, []);

  const saveConfig = async () => {
    await invoke("save_config", { newConfig: config });
  };

  return config && !isLoading ? (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        position: "absolute",
        top: 0,
        bottom: 0,
        left: 0,
        right: 0,
      }}
    >
      <div
        style={{
          backgroundColor: "#fff",
          display: "flex",
          flexDirection: "row",
          borderBottom: "solid #888 1px",
          padding: 10,
          zIndex: 10,
          boxShadow: "0 2px 5px rgb(152, 151, 151)",
          justifyContent: "space-between",
          marginBottom: 10,
        }}
      >
        <div style={{ display: "flex", alignItems: "center" }}>
          <GestureIcon />
          <Typography style={{ marginLeft: 10, marginRight: 10 }}>
            Shape button :
          </Typography>
          <ButtonSelector
            button={config.shape_button}
            setButton={setShapeButton}
          />
        </div>
        <ButtonGroup>
          <Button
            color="warning"
            variant="contained"
            onClick={() => invoke("stop")}
          >
            <StopIcon /> Stop
          </Button>
          <Button
            variant="contained"
            onClick={() => invoke("start")}
            color="success"
          >
            <PlayArrowIcon /> Start
          </Button>
          <Button color="warning" variant="contained" onClick={refreshConfig}>
            <UndoIcon /> Reload config
          </Button>
          <Button variant="contained" onClick={saveConfig}>
            <SaveIcon /> Save
          </Button>
        </ButtonGroup>
      </div>
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          overflow: "auto",
        }}
      >
        {config.bindings.map((binding, index) => (
          <BindingMemo
            key={binding.uid || index}
            binding={binding}
            setBinding={onNewBinding}
            addBinding={addBinding}
            deleteBinding={deleteBinding}
          />
        ))}
        <div
          style={{
            width: "100%",
            paddingBottom: 8,
            marginBottom: 8,
            display: "flex",
            justifyContent: "center",
          }}
        >
          <Button
            variant="contained"
            size="small"
            onClick={() =>
              addBinding(config?.bindings[config.bindings.length - 1])
            }
          >
            <AddIcon /> Add a binding
          </Button>
        </div>

        <Divider style={{ width: "100%", margin: "16px 0" }} />
        <Typography variant="h6" style={{ marginBottom: 8 }}>
          Modifier remaps
        </Typography>
        <Typography
          variant="body2"
          style={{ color: "#666", marginBottom: 10, maxWidth: 720, textAlign: "center" }}
        >
          While the gate input is held, the trigger input is intercepted and
          the configured key/button is emitted instead. "Toggle" makes each
          trigger press flip the emit on/off; "Hold" releases when the trigger
          releases.
        </Typography>
        {(config.modifier_remaps ?? []).map((remap) => (
          <ModifierRemapMemo
            key={remap.uid}
            remap={remap}
            setRemap={setModifierRemap}
            addRemap={addModifierRemap}
            deleteRemap={deleteModifierRemap}
          />
        ))}
        <div
          style={{
            width: "100%",
            paddingBottom: 8,
            marginBottom: 8,
            display: "flex",
            justifyContent: "center",
          }}
        >
          <Button
            variant="contained"
            size="small"
            onClick={() => addModifierRemap()}
          >
            <AddIcon /> Add a modifier remap
          </Button>
        </div>

        <Divider style={{ width: "100%", margin: "16px 0" }} />
        <Typography variant="h6" style={{ marginBottom: 8 }}>
          Chord bindings
        </Typography>
        <Typography
          variant="body2"
          style={{ color: "#666", marginBottom: 10, maxWidth: 720, textAlign: "center" }}
        >
          When all selected mouse buttons are pressed within the time window,
          the command runs. Pass-through keeps original button events flowing
          to the host app (so e.g. browser back/forward still works).
        </Typography>
        {(config.chord_bindings ?? []).map((chord) => (
          <ChordBindingMemo
            key={chord.uid}
            chord={chord}
            setChord={setChordBinding}
            addChord={addChordBinding}
            deleteChord={deleteChordBinding}
          />
        ))}
        <div
          style={{
            width: "100%",
            paddingBottom: 8,
            marginBottom: 16,
            display: "flex",
            justifyContent: "center",
          }}
        >
          <Button
            variant="contained"
            size="small"
            onClick={() => addChordBinding()}
          >
            <AddIcon /> Add a chord binding
          </Button>
        </div>
      </div>
    </div>
  ) : (
    <AppSkeleton />
  );
}
