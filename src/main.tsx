import { createRoot } from "react-dom/client";
import App from "./App";
import { CaptureRegionWindow, SnapshotConfirmationWindow } from "./CaptureRegionWindow";
import CaptureBarWindow from "./CaptureBarWindow";
import { TooltipProvider } from "./components/ui/Tooltip";
import "./styles/font-ranges.css";
import "./styles/tokens.css";
import "./style.css";

if (window.location.hash === "#capture-region" || window.location.hash === "#snapshot-confirmation" || window.location.hash === "#capture-bar") {
  document.body.style.background = "transparent";
  document.documentElement.style.background = "transparent";
}

const content = window.location.hash === "#capture-region"
  ? <CaptureRegionWindow />
  : window.location.hash === "#capture-bar"
    ? <CaptureBarWindow />
    : window.location.hash === "#snapshot-confirmation"
      ? <SnapshotConfirmationWindow />
      : <TooltipProvider><App /></TooltipProvider>;

createRoot(document.getElementById("root")!).render(content);
