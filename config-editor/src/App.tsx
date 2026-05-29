import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
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
import {
  Button,
  ButtonGroup,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  Divider,
  Tooltip,
  Typography,
} from "@mui/material";
import PlayArrowIcon from "@mui/icons-material/PlayArrow";
import StopIcon from "@mui/icons-material/Stop";
import RestartAltIcon from "@mui/icons-material/RestartAlt";
import SaveIcon from "@mui/icons-material/Save";
import UndoIcon from "@mui/icons-material/Undo";
import GestureIcon from "@mui/icons-material/Gesture";
import AddIcon from "@mui/icons-material/Add";
import InstallDesktopIcon from "@mui/icons-material/InstallDesktop";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import { AppSkeleton } from "./AppSkeleton";

type ServiceStatus = {
  available: boolean;
  active: boolean;
  active_state: string;
  sub_state: string;
  fragment_path: string;
  user_installed: boolean;
};

const POLL_MS = 2500;

export default function App() {
  const [isLoading, setIsLoading] = useState(false);
  const [defaultConfigPath, setGreetMsg] = useState("");
  const [version, setVersion] = useState("");
  const [config, setConfig] = useState<ConfigType>();
  const [shapeRecording, setShapeRecording] = useState(false);
  const [svc, setSvc] = useState<ServiceStatus | undefined>();
  const [confirm, setConfirm] = useState<
    | { kind: "install" }
    | { kind: "uninstall" }
    | { kind: "error"; message: string }
    | undefined
  >();

  async function getDefaultConfigPath() {
    // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
    setGreetMsg(await invoke("get_default_config_path"));
  }

  useEffect(() => {
    invoke("get_version").then((v: any) => setVersion(v));
  }, []);

  // --- Service status polling --------------------------------------------
  //
  // Hits the Tauri command which shells out to `systemctl --user show`. Used
  // to color the status chip and gate the Start/Stop buttons. Best-effort —
  // errors silently leave the chip showing the last known state.
  const refreshSvc = useCallback(async () => {
    try {
      const s: ServiceStatus = await invoke("service_status");
      setSvc(s);
    } catch (e) {
      // Tauri command failed — treat the service as unmanaged.
      setSvc(undefined);
    }
  }, []);

  useEffect(() => {
    refreshSvc();
    const t = setInterval(refreshSvc, POLL_MS);
    return () => clearInterval(t);
  }, [refreshSvc]);

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

  // All binding mutators below MUST spread `prevConfig` first. Building a
  // fresh `{ shape_button, bindings }` literal silently strips every
  // sibling field — modifier_remaps, chord_bindings, anything we add
  // later — and the next Save then writes the truncated config to disk.
  const onNewBinding = useCallback(
    (newBinding: BindingType) => {
      setConfig((prevConfig) => {
        if (!prevConfig) return prevConfig;
        const bindings = [...prevConfig.bindings];
        const index = bindings.findIndex((b) => b.uid === newBinding.uid);
        if (index >= 0) bindings[index] = newBinding;
        return { ...prevConfig, bindings };
      });
    },
    [setConfig]
  );

  const deleteBinding = useCallback(
    (binding: BindingType) => {
      setConfig((prevConfig) => {
        if (!prevConfig) return prevConfig;
        const bindings = [...prevConfig.bindings];
        const index = bindings.findIndex((b) => b.uid === binding.uid);
        if (index >= 0) bindings.splice(index, 1);
        return { ...prevConfig, bindings };
      });
    },
    [setConfig]
  );

  const addBinding = useCallback(
    (binding?: BindingType) => {
      setConfig((prevConfig) => {
        if (!prevConfig) return prevConfig;
        const bindings = [...prevConfig.bindings];
        const index = binding
          ? bindings.findIndex((b) => b.uid === binding.uid)
          : -1;
        bindings.splice((index ?? -1) + 1, 0, {
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
        return { ...prevConfig, bindings };
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

  // When the systemd unit is loaded, route the toolbar buttons through it so
  // the GUI and the tray (and `systemctl status`) all agree about who's
  // running. When the unit isn't present, fall back to the original direct
  // subprocess path so the GUI still works on machines without systemd
  // integration.
  const useService = !!svc?.available;
  const onStart = useCallback(async () => {
    await invoke(useService ? "service_start" : "start");
    refreshSvc();
  }, [useService, refreshSvc]);
  const onStop = useCallback(async () => {
    await invoke(useService ? "service_stop" : "stop");
    refreshSvc();
  }, [useService, refreshSvc]);
  const onRestart = useCallback(async () => {
    await invoke("service_restart");
    refreshSvc();
  }, [refreshSvc]);

  const onInstallConfirm = useCallback(async () => {
    setConfirm(undefined);
    try {
      await invoke("service_install");
    } catch (e) {
      setConfirm({ kind: "error", message: String(e) });
    } finally {
      refreshSvc();
    }
  }, [refreshSvc]);

  const onUninstallConfirm = useCallback(async () => {
    setConfirm(undefined);
    try {
      await invoke("service_uninstall");
    } catch (e) {
      setConfirm({ kind: "error", message: String(e) });
    } finally {
      refreshSvc();
    }
  }, [refreshSvc]);

  const svcChipLabel = svc
    ? svc.available
      ? `service: ${svc.active_state}${svc.sub_state ? ` (${svc.sub_state})` : ""}`
      : "service: not installed"
    : "service: ?";
  const svcChipColor: "success" | "default" | "warning" | "error" =
    svc?.active
      ? "success"
      : svc?.available && svc.active_state === "failed"
        ? "error"
        : svc?.available
          ? "warning"
          : "default";
  const startDisabled = useService && !!svc?.active;
  const stopDisabled = useService && !svc?.active;

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
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <Tooltip
            title={
              useService
                ? "Buttons act on the systemd user unit mouse-actions.service. Polled every 2.5 s."
                : "mouse-actions.service is not installed; Start/Stop spawn the daemon directly as a subprocess."
            }
          >
            <Chip
              size="small"
              color={svcChipColor}
              variant={svcChipColor === "default" ? "outlined" : "filled"}
              label={svcChipLabel}
            />
          </Tooltip>
          {svc && !svc.available && (
            <Button
              size="small"
              variant="outlined"
              color="primary"
              onClick={() => setConfirm({ kind: "install" })}
              startIcon={<InstallDesktopIcon />}
            >
              Install service
            </Button>
          )}
          {svc?.available && svc.user_installed && (
            <Tooltip
              title={`Will disable, stop, and remove ${svc.fragment_path}.`}
            >
              <Button
                size="small"
                variant="outlined"
                color="warning"
                onClick={() => setConfirm({ kind: "uninstall" })}
                startIcon={<DeleteForeverIcon />}
              >
                Uninstall
              </Button>
            </Tooltip>
          )}
          <ButtonGroup>
            <Button
              color="warning"
              variant="contained"
              onClick={onStop}
              disabled={stopDisabled}
            >
              <StopIcon /> Stop
            </Button>
            <Button
              variant="contained"
              onClick={onStart}
              color="success"
              disabled={startDisabled}
            >
              <PlayArrowIcon /> Start
            </Button>
            {useService && (
              <Button variant="contained" onClick={onRestart} color="info">
                <RestartAltIcon /> Restart
              </Button>
            )}
            <Button color="warning" variant="contained" onClick={refreshConfig}>
              <UndoIcon /> Reload config
            </Button>
            <Button variant="contained" onClick={saveConfig}>
              <SaveIcon /> Save
            </Button>
          </ButtonGroup>
        </div>
        <Dialog
          open={confirm?.kind === "install"}
          onClose={() => setConfirm(undefined)}
        >
          <DialogTitle>Install mouse-actions.service?</DialogTitle>
          <DialogContent>
            <DialogContentText component="div">
              Writes a systemd user unit to
              <pre style={{ margin: "6px 0", background: "#f4f4f4", padding: 6 }}>
                ~/.config/systemd/user/mouse-actions.service
              </pre>
              and runs:
              <pre style={{ margin: "6px 0", background: "#f4f4f4", padding: 6 }}>
                {"systemctl --user daemon-reload\nsystemctl --user enable --now mouse-actions.service"}
              </pre>
              The unit's ExecStart points at the `mouse-actions` CLI binary
              found next to this app (or on PATH). After install, this GUI's
              Start/Stop/Restart buttons route through systemd.
            </DialogContentText>
          </DialogContent>
          <DialogActions>
            <Button onClick={() => setConfirm(undefined)}>Cancel</Button>
            <Button
              variant="contained"
              color="primary"
              onClick={onInstallConfirm}
            >
              Install
            </Button>
          </DialogActions>
        </Dialog>
        <Dialog
          open={confirm?.kind === "uninstall"}
          onClose={() => setConfirm(undefined)}
        >
          <DialogTitle>Uninstall mouse-actions.service?</DialogTitle>
          <DialogContent>
            <DialogContentText component="div">
              Will run:
              <pre style={{ margin: "6px 0", background: "#f4f4f4", padding: 6 }}>
                {`systemctl --user disable --now mouse-actions.service\nrm ${svc?.fragment_path ?? ""}\nsystemctl --user daemon-reload`}
              </pre>
              The mouse-actions CLI binary itself is untouched. Only the user
              systemd unit file installed by this GUI is removed.
            </DialogContentText>
          </DialogContent>
          <DialogActions>
            <Button onClick={() => setConfirm(undefined)}>Cancel</Button>
            <Button
              variant="contained"
              color="warning"
              onClick={onUninstallConfirm}
            >
              Uninstall
            </Button>
          </DialogActions>
        </Dialog>
        <Dialog
          open={confirm?.kind === "error"}
          onClose={() => setConfirm(undefined)}
        >
          <DialogTitle>Service action failed</DialogTitle>
          <DialogContent>
            <DialogContentText
              component="pre"
              style={{ whiteSpace: "pre-wrap", fontFamily: "monospace" }}
            >
              {confirm?.kind === "error" ? confirm.message : ""}
            </DialogContentText>
          </DialogContent>
          <DialogActions>
            <Button onClick={() => setConfirm(undefined)}>OK</Button>
          </DialogActions>
        </Dialog>
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
