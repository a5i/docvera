import type { Metadata } from "next";
import "@fontsource-variable/geist";
import "@fontsource-variable/geist-mono";
import "./globals.css";
import { Providers } from "@/components/providers";
import { Header, Footer } from "@/components/site-shell";

export const metadata: Metadata = {
  title: { default: "docvera — Markdown, with guarantees.", template: "%s · docvera" },
  description: "Turn Git-managed Markdown into a validated, typed, queryable knowledge base. Explore the docs and run the real Rust engine in your browser.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return <html lang="en" suppressHydrationWarning>
    <body><Providers>
      <a className="skip-link" href="#main">Skip to content</a>
      <Header />
      {children}
      <Footer />
    </Providers></body>
  </html>;
}
