// Builds what the site takes from the repository and from its own home page:
// the icon sprite, the changelog and documentation pages from the markdown
// beside the code, a markdown version of every page, and the files crawlers
// and language models look for. The pages borrow the header and the footer
// of public/index.html.
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { marked } from "marked";
import { parse } from "node-html-parser";

const here = new URL(".", import.meta.url);
const read = (path) => readFileSync(new URL(path, here), "utf8");
const write = (path, text) => {
  const url = new URL(path, here);
  mkdirSync(new URL(".", url), { recursive: true });
  writeFileSync(url, text);
};

const site = "https://clauset.app";
const repository = "https://github.com/mischasigtermans/clauset";

// Phosphor's icons (MIT), as one sprite the pages reference by name.
const icons = [
  "app-window",
  "gauge",
  "browser",
  "power",
  "stethoscope",
  "terminal-window",
  "magic-wand",
  "folder-open",
  "robot",
  "brackets-curly",
  "arrow-u-up-left",
  "scales",
];
const symbols = icons.map((name) => {
  const svg = read(`node_modules/@phosphor-icons/core/assets/regular/${name}.svg`);
  const paths = svg.replace(/^<svg[^>]*>/, "").replace(/<\/svg>\s*$/, "");
  return `<symbol id="${name}" viewBox="0 0 256 256">${paths}</symbol>`;
});
write("public/img/icons.svg", `<svg xmlns="http://www.w3.org/2000/svg">${symbols.join("")}</svg>\n`);

// A heading's anchor, as GitHub makes one, so links written for the
// repository still land.
const anchor = (text) =>
  text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s-]/gu, "")
    .trim()
    .replace(/\s+/g, "-");

// Where a link in the markdown leads on the site: the two pages to each
// other, anything else in the repository to GitHub.
function destination(href) {
  if (/^(https?:|mailto:|#)/.test(href)) return href;
  const [path, hash] = href.replace(/^(\.\.\/|\.\/)+/, "").split("#");
  const tail = hash ? `#${hash}` : "";
  if (path === "docs/configuration.md" || path === "configuration.md") return `../docs/${tail}`;
  if (path === "CHANGELOG.md") return `../changelog/${tail}`;
  return `${repository}/blob/main/${path}${tail}`;
}

marked.use({
  renderer: {
    heading({ tokens, depth, text }) {
      return `<h${depth} id="${anchor(text)}">${this.parser.parseInline(tokens)}</h${depth}>\n`;
    },
    link({ href, tokens }) {
      const to = destination(href);
      const outside = /^https?:/.test(to) ? ' target="_blank" rel="noopener"' : "";
      return `<a href="${to}"${outside}>${this.parser.parseInline(tokens)}</a>`;
    },
  },
});

// The questions on the home page, repeated as structured data. Written into
// the page between two markers, so what a crawler reads is what a visitor
// reads.
let index = read("public/index.html");
const home = parse(index);
const questions = home.querySelectorAll("[data-faq] > div").map((item) => ({
  "@type": "Question",
  name: item.querySelector("dt").text.replace(/\s+/g, " ").trim(),
  acceptedAnswer: { "@type": "Answer", text: item.querySelector("dd").text.replace(/\s+/g, " ").trim() },
}));
const faq = JSON.stringify({ "@context": "https://schema.org", "@type": "FAQPage", mainEntity: questions });
index = index.replace(
  /<!-- faq:start -->[\s\S]*?<!-- faq:end -->/,
  `<!-- faq:start -->\n    <script type="application/ld+json">${faq}</script>\n    <!-- faq:end -->`,
);
write("public/index.html", index);

const under = (html) =>
  html
    .replaceAll('href="./"', 'href="../"')
    .replaceAll('src="img/', 'src="../img/')
    .replaceAll('href="#', 'href="../#')
    .replaceAll('href="changelog/"', 'href="../changelog/"')
    .replaceAll('href="docs/"', 'href="../docs/"');
const header = under(index.match(/<header[\s\S]*?<\/header>/)[0]);
const footer = under(index.match(/<footer[\s\S]*?<\/footer>/)[0]);

const pages = [
  {
    path: "",
    title: home.querySelector("title").text,
    description: home.querySelector('meta[name="description"]').getAttribute("content"),
  },
  {
    path: "docs/",
    title: "Documentation",
    description: "The config file, how an account is picked, what's shared and what Clauset writes.",
    source: "../docs/configuration.md",
  },
  {
    path: "changelog/",
    title: "Changelog",
    description: "What changed in each version of Clauset, the menu bar app and the command, newest first.",
    source: "../CHANGELOG.md",
  },
];

function page({ path, title, description }, markdown) {
  const body = marked
    .parse(markdown)
    .replaceAll("<table>", '<div class="table-scroll"><table>')
    .replaceAll("</table>", "</table></div>");
  return `<!doctype html>
<html lang="en" class="scroll-smooth antialiased motion-reduce:scroll-auto">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <meta name="color-scheme" content="light dark" />
    <title>${title} | Clauset</title>
    <meta name="description" content="${description}" />
    <link rel="canonical" href="${site}/${path}" />
    <link rel="alternate" type="text/markdown" href="${site}/${path}index.md" />
    <meta property="og:site_name" content="Clauset" />
    <meta property="og:type" content="article" />
    <meta property="og:url" content="${site}/${path}" />
    <meta property="og:locale" content="en_GB" />
    <meta property="og:title" content="${title} | Clauset" />
    <meta property="og:description" content="${description}" />
    <meta property="og:image" content="${site}/img/og.png" />
    <meta property="og:image:width" content="1200" />
    <meta property="og:image:height" content="630" />
    <meta property="og:image:alt" content="Clauset: every Claude Code account, in your menu bar" />
    <meta name="twitter:card" content="summary_large_image" />
    <meta name="twitter:image" content="${site}/img/og.png" />
    <meta name="theme-color" content="#f4f5fa" media="(prefers-color-scheme: light)" />
    <meta name="theme-color" content="#0d1017" media="(prefers-color-scheme: dark)" />
    <link rel="apple-touch-icon" href="../img/apple-touch-icon.png" />
    <link rel="icon" href="../img/favicon.png" />
    <link rel="stylesheet" href="../assets/site.css" />
  </head>
  <body class="bg-page text-ink dark:bg-night dark:text-fog">
    <div class="isolate">
      ${header}
      <main class="py-16 sm:py-24">
        <div class="mx-auto max-w-6xl px-6 lg:px-8">
          <article class="prose">
${body}
          </article>
        </div>
      </main>
      ${footer}
    </div>
  </body>
</html>
`;
}

// A link of the home page, as an address that holds outside it.
const absolute = (href) => (/^(https?:|mailto:)/.test(href) ? href : `${site}/${href.replace(/^\.\//, "")}`);

// The home page as markdown: what it says, without the pictures.
function markdownOfHome() {
  // The parser keeps a `pre` as raw text unless told otherwise, markup and all.
  const main = parse(index, { blockTextElements: { script: true, style: true } }).querySelector("main");
  main.querySelectorAll('[aria-hidden="true"], picture, img, svg').forEach((node) => node.remove());
  const inline = (node) =>
    node.childNodes
      .map((child) => {
        if (child.nodeType === 3) return child.text;
        const tag = child.rawTagName?.toLowerCase();
        if (tag === "code") return `\`${child.text.trim()}\``;
        if (tag === "a") return `[${inline(child)}](${absolute(child.getAttribute("href"))})`;
        return inline(child);
      })
      .join("")
      .replace(/\s+/g, " ")
      .trim();
  const blocks = [];
  let actions = [];
  const flush = () => {
    if (actions.length) blocks.push(actions.join(" | "));
    actions = [];
  };
  for (const node of main.querySelectorAll("h1, h2, dt, p, dd, pre, a")) {
    const tag = node.rawTagName.toLowerCase();
    if (tag === "a") {
      if (!node.closest("p, dd, dt, h1, h2, pre")) actions.push(`[${inline(node)}](${absolute(node.getAttribute("href"))})`);
      continue;
    }
    flush();
    if (tag === "h1") blocks.push(`# ${inline(node)}`);
    else if (tag === "h2") blocks.push(`## ${inline(node)}`);
    else if (tag === "dt") blocks.push(`### ${inline(node)}`);
    else if (tag === "pre") blocks.push(`\`\`\`\n${node.text.replace(/\s+$/, "")}\n\`\`\``);
    else blocks.push(inline(node));
  }
  flush();
  return `${blocks.join("\n\n")}\n`;
}

// The repository's markdown with its links pointing at the site.
const markdownOf = (source) =>
  read(source).replace(/\]\(([^)\s]+)\)/g, (_, href) => {
    const to = destination(href);
    return `](${to.startsWith("../") ? `${site}/${to.slice(3)}` : to})`;
  });

const markdown = {};
for (const entry of pages) {
  markdown[entry.path] = entry.source ? markdownOf(entry.source) : markdownOfHome();
  write(`public/${entry.path}index.md`, markdown[entry.path]);
  if (entry.source) write(`public/${entry.path}index.html`, page(entry, read(entry.source)));
}

const summary =
  "Clauset is a free, open-source menu bar app and command for macOS. It runs several Claude Code accounts on one Mac, sharing one ~/.claude, shows each account's usage limits, and lets you move a running session to another account when a limit nears. It never switches accounts by itself.";
const listed = pages
  .map((entry) => `- [${entry.title}](${site}/${entry.path}index.md): ${entry.description}`)
  .join("\n");
const llms = `# Clauset

> ${summary}

## Pages

${listed}

## Source

- [GitHub repository](${repository}): the app, the command and this site, under the MIT licence.
`;
write("public/llms.txt", llms);
write(
  "public/llms-full.txt",
  `${llms}\n${pages.map((entry) => `---\n\nSource: ${site}/${entry.path}\n\n${markdown[entry.path]}`).join("\n")}`,
);

write("public/robots.txt", `User-agent: *\nAllow: /\n\nSitemap: ${site}/sitemap.xml\n`);
write(
  "public/sitemap.xml",
  `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${pages.map((entry) => `  <url>\n    <loc>${site}/${entry.path}</loc>\n  </url>`).join("\n")}
</urlset>
`,
);
