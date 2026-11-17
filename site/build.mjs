// Builds what the site takes from the repository: the icon sprite, and the
// changelog and documentation pages from the markdown beside the code. The
// pages borrow the header and the footer of public/index.html.
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { marked } from "marked";

const here = new URL(".", import.meta.url);
const read = (path) => readFileSync(new URL(path, here), "utf8");
const write = (path, text) => {
  const url = new URL(path, here);
  mkdirSync(new URL(".", url), { recursive: true });
  writeFileSync(url, text);
};

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

const index = read("public/index.html");
const under = (html) =>
  html
    .replaceAll('href="./"', 'href="../"')
    .replaceAll('src="img/', 'src="../img/')
    .replaceAll('href="#', 'href="../#')
    .replaceAll('href="changelog/"', 'href="../changelog/"')
    .replaceAll('href="docs/"', 'href="../docs/"');
const header = under(index.match(/<header[\s\S]*?<\/header>/)[0]);
const footer = under(index.match(/<footer[\s\S]*?<\/footer>/)[0]);

function page(path, title, description, markdown) {
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
    <link rel="canonical" href="https://clauset.app/${path}/" />
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

write(
  "public/changelog/index.html",
  page("changelog", "Changelog", "What changed in each version of Clauset.", read("../CHANGELOG.md")),
);
write(
  "public/docs/index.html",
  page(
    "docs",
    "Documentation",
    "The config file, how an account is picked, what's shared and what clauset writes.",
    read("../docs/configuration.md"),
  ),
);
