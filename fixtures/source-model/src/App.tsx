import { BrowserRouter, Route, Routes } from "react-router";
import { Orders } from "./Orders";
import { Settings } from "./Settings";
import "./theme.css";

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Orders />} />
        <Route path="/settings" element={<Settings />} />
      </Routes>
    </BrowserRouter>
  );
}
