import type { MDXComponents } from "mdx/types";
import { Playground } from "@/components/playground";
import { headingId } from "@/lib/headings";

export function useMDXComponents(components: MDXComponents): MDXComponents {
  return {
    Playground,
    h2: ({ children, ...props }) => <h2 id={headingId(String(children))} {...props}>{children}</h2>,
    ...components,
  };
}
