import ReactDOM from "react-dom/client";
import App from "./App";
import { ApprovalWindow } from "./ui/ApprovalWindow";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  location.search.includes("approval") ? <ApprovalWindow /> : <App />,
);
