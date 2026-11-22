import React from "react";
import {
  LayoutDashboard,
  FilePlus2,
  FolderOutput,
  FolderTree,
  Sliders,
  Activity,
  Gauge,
  Settings,
  Info,
  Sun,
  Moon,
} from "lucide-react";
import { NavSection } from "../types/archive";

interface SidebarProps {
  activeSection: NavSection;
  onSelectSection: (section: NavSection) => void;
  theme: "dark" | "light";
  onToggleTheme: () => void;
  engineVersion: string;
}

export const Sidebar: React.FC<SidebarProps> = ({
  activeSection,
  onSelectSection,
  theme,
  onToggleTheme,
  engineVersion,
}) => {
  const navItems: {
    id: NavSection;
    label: string;
    group: "Core" | "Engine" | "System";
    icon: React.ReactNode;
  }[] = [
    { id: "dashboard", label: "Home / Dashboard", group: "Core", icon: <LayoutDashboard size={17} /> },
    { id: "create", label: "Create Archive", group: "Core", icon: <FilePlus2 size={17} /> },
    { id: "extract", label: "Extract Archive", group: "Core", icon: <FolderOutput size={17} /> },
    { id: "explorer", label: "Archive Explorer", group: "Core", icon: <FolderTree size={17} /> },
    {
      id: "compression-settings",
      label: "Compression Settings",
      group: "Engine",
      icon: <Sliders size={17} />,
    },
    { id: "performance", label: "Performance Monitor", group: "Engine", icon: <Activity size={17} /> },
    { id: "benchmark", label: "Benchmark Center", group: "Engine", icon: <Gauge size={17} /> },
    { id: "settings", label: "Settings", group: "System", icon: <Settings size={17} /> },
    { id: "about", label: "About Kinetic", group: "System", icon: <Info size={17} /> },
  ];

  const renderGroup = (groupName: "Core" | "Engine" | "System", title: string) => (
    <div key={groupName}>
      <div className="nav-group-label">{title}</div>
      {navItems
        .filter((item) => item.group === groupName)
        .map((item) => (
          <button
            key={item.id}
            className={`nav-item ${activeSection === item.id ? "active" : ""}`}
            onClick={() => onSelectSection(item.id)}
          >
            {item.icon}
            <span>{item.label}</span>
          </button>
        ))}
    </div>
  );

  return (
    <aside className="sidebar">
      <div>
        <div className="brand-box">
          <div className="brand-logo">K</div>
          <div>
            <div className="brand-title">KINETIC</div>
            <div className="brand-subtitle">Archive Manager v{engineVersion}</div>
          </div>
        </div>

        <nav className="nav-list">
          {renderGroup("Core", "Archive Operations")}
          {renderGroup("Engine", "Kinetic Engine")}
          {renderGroup("System", "Application")}
        </nav>
      </div>

      <div className="sidebar-footer">
        <button className="btn" onClick={onToggleTheme} style={{ width: "100%" }}>
          {theme === "dark" ? <Sun size={15} /> : <Moon size={15} />}
          <span>{theme === "dark" ? "Switch to Light Mode" : "Switch to Dark Mode"}</span>
        </button>
      </div>
    </aside>
  );
};