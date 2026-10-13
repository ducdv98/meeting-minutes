import { createRoot } from "react-dom/client";
import App from "./App";
import { TooltipProvider } from "./components/ui/Tooltip";
import "./styles/font-ranges.css";
import "./styles/tokens.css";
import "./style.css";

createRoot(document.getElementById("root")!).render(<TooltipProvider><App /></TooltipProvider>);
