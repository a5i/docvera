"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { ArrowUpRight, FlaskConical } from "lucide-react";
import { docs, docHref } from "@/lib/navigation";

export function DocsSidebar() {
  const path = usePathname().replace(/\/$/, "");
  const groups = [...new Set(docs.map(item => item.group))];
  return <aside className="docs-sidebar"><div className="sidebar-label">DOCUMENTATION <span>v0.2</span></div><nav aria-label="Documentation">{groups.map(group => <div className="sidebar-group" key={group}><h2>{group}</h2>{docs.filter(item => item.group === group).map(item => <Link key={item.slug} aria-current={path === docHref(item.slug).replace(/\/$/, "") ? "page" : undefined} href={docHref(item.slug)}>{item.title}</Link>)}</div>)}</nav><Link className="sidebar-playground" href="/playground/"><FlaskConical size={19} /><strong>Learn by changing.</strong><span>Try the live playground <ArrowUpRight size={12} /></span></Link></aside>;
}
