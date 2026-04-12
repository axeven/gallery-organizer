import { NavLink } from "react-router-dom";

const navItems = [
  { to: "/scan", label: "Scan", icon: "⬡" },
  { to: "/gallery", label: "Gallery", icon: "◫" },
  { to: "/duplicates", label: "Duplicates", icon: "⊞" },
  { to: "/jobs", label: "Jobs", icon: "▷" },
  { to: "/settings", label: "Settings", icon: "⚙" },
];

export default function Sidebar() {
  return (
    <aside className="w-48 bg-neutral-900 border-r border-neutral-800 flex flex-col py-4 gap-1">
      <div className="px-4 pb-4 text-lg font-bold tracking-tight text-white">
        Gallery Org
      </div>
      {navItems.map((item) => (
        <NavLink
          key={item.to}
          to={item.to}
          className={({ isActive }) =>
            `flex items-center gap-3 px-4 py-2 text-sm rounded-lg mx-2 transition-colors ${
              isActive
                ? "bg-neutral-700 text-white"
                : "text-neutral-400 hover:text-white hover:bg-neutral-800"
            }`
          }
        >
          <span>{item.icon}</span>
          <span>{item.label}</span>
        </NavLink>
      ))}
    </aside>
  );
}
