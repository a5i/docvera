"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useState } from "react";
import { useTheme } from "next-themes";
import { ArrowUpRight, Check, Command, FileCheck2, GitFork, Menu, Moon, Search, Sun } from "lucide-react";
import { Button, buttonVariants } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { docs, docHref } from "@/lib/navigation";
import { cn } from "@/lib/utils";

export function Logo() {
  return <Link href="/" className="wordmark" aria-label="docvera home"><span className="logo-mark"><FileCheck2 size={21} strokeWidth={1.8} /></span>docvera<span className="wordmark-dot">.</span></Link>;
}

function ThemeToggle() {
  const { resolvedTheme, setTheme } = useTheme();
  return <Button variant="ghost" size="icon" className="theme-toggle" aria-label="Toggle day / night theme" onClick={() => setTheme(resolvedTheme === "dark" ? "light" : "dark")}>
    <Sun className="sun-icon" /><Moon className="moon-icon" />
  </Button>;
}

function SearchDialog() {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key === "k") { event.preventDefault(); setOpen(value => !value); }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  const results = docs.filter(item => `${item.title} ${item.description}`.toLowerCase().includes(query.toLowerCase()));
  return <Dialog open={open} onOpenChange={setOpen}>
    <DialogTrigger render={<Button variant="outline" className="search-trigger" aria-label="Search documentation" />}><Search size={15} /><span>Search docs...</span><kbd><Command size={11} /> K</kbd></DialogTrigger>
    <DialogContent className="search-dialog sm:max-w-lg">
      <DialogTitle>Find your way</DialogTitle>
      <DialogDescription>Search documentation topics.</DialogDescription>
      <input autoFocus className="search-input" aria-label="Search documentation" placeholder="Schemas, SQL, browser…" value={query} onChange={event => setQuery(event.target.value)} />
      <div className="search-results">{results.map(item => <Link key={item.slug} href={docHref(item.slug)} onClick={() => setOpen(false)}><span>{item.title}</span><small>{item.description}</small></Link>)}{!results.length && <p>No matching pages. Try “schema” or “SQL”.</p>}</div>
    </DialogContent>
  </Dialog>;
}

export function Header() {
  const path = usePathname();
  const [menuOpen, setMenuOpen] = useState(false);
  return <header className="site-header"><div className="header-inner">
    <Logo />
    <nav className="desktop-nav" aria-label="Main navigation">
      <Link data-active={path.startsWith("/docs")} href="/docs/">Documentation</Link>
      <Link data-active={path === "/playground/" || path === "/playground"} href="/playground/">Playground <span className="tiny-label">LIVE</span></Link>
    </nav>
    <div className="header-actions"><SearchDialog /><span className="header-divider" /><ThemeToggle /><a className="github-link" href="https://github.com/a5i/docvera" aria-label="docvera on GitHub"><GitFork size={19} /></a>
    <Button variant="ghost" size="icon" className="mobile-menu-button" aria-label="Toggle navigation" aria-expanded={menuOpen} onClick={() => setMenuOpen(!menuOpen)}><Menu /></Button></div>
  </div>{menuOpen && <nav className="mobile-nav" aria-label="Mobile navigation"><Link href="/docs/" onClick={() => setMenuOpen(false)}>Documentation</Link><Link href="/playground/" onClick={() => setMenuOpen(false)}>Playground</Link></nav>}</header>;
}

export function Footer() {
  return <footer className="site-footer"><div className="footer-inner"><div><Logo /><p>Plain text. Strong foundations.</p></div><div className="footer-links"><Link href="/docs/">Documentation</Link><a href="https://github.com/a5i/docvera">Source <ArrowUpRight size={13} /></a><a href="https://github.com/a5i/docvera/blob/main/LICENSE">MIT license</a></div><span className="footer-note"><span className="status-dot" /> Built with Rust. Made for Markdown.</span></div></footer>;
}

export function CopyCommand({ command = "npm install @a5i/docvera" }: { command?: string }) {
  const [copied, setCopied] = useState(false);
  const [failed, setFailed] = useState(false);
  async function copy() {
    try { await navigator.clipboard.writeText(command); setCopied(true); setTimeout(() => setCopied(false), 2000); }
    catch { setFailed(true); }
  }
  return <div className="install-command"><span aria-hidden="true">$</span><code>{command}</code><Button variant="ghost" size="icon-sm" aria-label="Copy install command" onClick={copy}>{copied ? <Check size={15} /> : <span className="copy-symbol">⧉</span>}</Button><span className="sr-only" role="status">{copied ? "Copied" : failed ? "Copy unavailable. Select the command to copy it." : ""}</span></div>;
}

export function ActionLink({ href, children, secondary = false }: { href: string; children: React.ReactNode; secondary?: boolean }) {
  return <Link className={cn(buttonVariants({ variant: secondary ? "outline" : "default", size: "lg" }), "action-link")} href={href}>{children}</Link>;
}
