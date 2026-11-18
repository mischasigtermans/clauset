import { createHash } from "node:crypto";
import { cpSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
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

cpSync(new URL("src/img/", here), new URL("public/img/", here), { recursive: true });
write("public/assets/site.js", read("src/site.js"));

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
  "copy",
  "check",
  "download-simple",
  "caret-down",
  "magnifying-glass",
  "book-open",
  "clock-counter-clockwise",
  "wifi-slash",
  "github-logo",
  "cloud",
  "gear",
  "file-text",
  "plugs",
  "brain",
  "key",
  "link-simple",
  "lock-key",
  "buildings",
  "star",
];
const symbols = icons.map((name) => {
  const svg = read(`node_modules/@phosphor-icons/core/assets/regular/${name}.svg`);
  const paths = svg.replace(/^<svg[^>]*>/, "").replace(/<\/svg>\s*$/, "");
  return `<symbol id="${name}" viewBox="0 0 256 256">${paths}</symbol>`;
});
// Claude's mark, from Simple Icons (CC0), beside the `claude` it stands for.
symbols.push(`<symbol id="claude" viewBox="0 0 24 24">${read("src/brands/claude.svg").match(/<path[^>]*>/)[0]}</symbol>`);
write("public/img/icons.svg", `<svg xmlns="http://www.w3.org/2000/svg">${symbols.join("")}</svg>\n`);

// A heading's anchor, as GitHub makes one, so links written for the
// repository still land.
const anchor = (text) =>
  text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s-]/gu, "")
    .trim()
    .replace(/\s+/g, "-");

// Where a link in a markdown file leads on the site: an article to its
// page, a category to its place on the documentation's first page, the
// changelog to its page, anything else in the repository to GitHub. `from`
// is the path in the repository of the file that holds the link.
function destination(href, from) {
  if (/^(https?:|mailto:|#)/.test(href)) return href;
  const url = new URL(href, `file:///${from}`);
  const path = url.pathname.slice(1);
  const article = path.match(/^docs\/([^/]+)\/([^/]+)\.md$/);
  if (article) return article[2] === "index" ? `/docs/#${article[1]}` : `/docs/${article[1]}/${article[2]}/${url.hash}`;
  if (path === "CHANGELOG.md") return `/changelog/${url.hash}`;
  return `${repository}/blob/main/${path}${url.hash}`;
}

let rendering = "";
marked.use({
  renderer: {
    heading({ tokens, depth, text }) {
      return `<h${depth} id="${anchor(text)}">${this.parser.parseInline(tokens)}</h${depth}>\n`;
    },
    link({ href, tokens }) {
      const to = destination(href, rendering);
      const outside = /^https?:/.test(to) ? ' target="_blank" rel="noopener"' : "";
      return `<a href="${to}"${outside}>${this.parser.parseInline(tokens)}</a>`;
    },
    // On a phone a table's rows stack, and a cell of a table with more
    // than two columns then needs its column's name with it.
    table({ header, rows }) {
      const cell = (tag, item, label) => `<${tag}${label ? ` data-label="${label}"` : ""}>${this.parser.parseInline(item.tokens)}</${tag}>`;
      const labels = header.map((item) => plain(item.text));
      const head = header.map((item) => cell("th", item)).join("");
      const body = rows.map((row) => `<tr>${row.map((item, at) => cell("td", item, header.length > 2 && at ? labels[at] : "")).join("")}</tr>`);
      return `<div class="table-scroll"><table>\n<thead><tr>${head}</tr></thead>\n<tbody>\n${body.join("\n")}\n</tbody>\n</table></div>\n`;
    },
  },
});
const html = (markdown, from) => {
  rendering = from;
  return marked.parse(markdown);
};
const inline = (text) => marked.parseInline(text).replaceAll("<code>", '<code class="code">');
const plain = (text) => text.replaceAll("`", "").replaceAll('"', "&quot;");

const absolute = (markdown, from) =>
  markdown.replace(/\]\(([^)\s]+)\)/g, (_, href) => {
    const to = destination(href, from);
    return `](${to.startsWith("/") ? site + to : to})`;
  });

function frontMatter(path) {
  const [, head, body] = read(`../${path}`).match(/^---\n([\s\S]*?)\n---\n?([\s\S]*)$/);
  const meta = Object.fromEntries(
    head.split("\n").map((line) => {
      const at = line.indexOf(":");
      return [
        line.slice(0, at).trim(),
        line
          .slice(at + 1)
          .trim()
          .replace(/^"(.*)"$/, "$1"),
      ];
    }),
  );
  return { path, body: body.trim(), title: meta.title, description: meta.description, order: Number(meta.order) };
}
const categories = readdirSync(new URL("../docs/", here), { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((folder) => {
    const files = readdirSync(new URL(`../docs/${folder.name}/`, here)).filter((name) => name.endsWith(".md"));
    const documents = files.map((name) => ({ slug: name.slice(0, -3), ...frontMatter(`docs/${folder.name}/${name}`) }));
    const index = documents.find((entry) => entry.slug === "index");
    const articles = documents
      .filter((entry) => entry !== index)
      .sort((a, b) => a.order - b.order)
      .map((entry) => ({
        ...entry,
        category: index.title,
        url: `/docs/${folder.name}/${entry.slug}/`,
        sections: marked
          .lexer(entry.body)
          .filter((token) => token.type === "heading" && token.depth === 2)
          .map((token) => ({ text: token.text, id: anchor(token.text) })),
      }));
    return { slug: folder.name, title: index.title, description: index.description, order: index.order, articles };
  })
  .sort((a, b) => a.order - b.order);
const articles = categories.flatMap((category) => category.articles);

const changelog = read("../CHANGELOG.md").replace(/^# .*\n+/, "");
const day = (date) => new Date(date).toLocaleDateString("en-GB", { day: "numeric", month: "long", year: "numeric", timeZone: "UTC" });
const versions = [...changelog.matchAll(/^## \[([^\]]+)\](?: - (\d{4}-\d{2}-\d{2}))?$/gm)].map(([, name, date]) => ({
  name,
  date,
  id: name === "Unreleased" ? "unreleased" : `v${name}`,
}));

// The questions on the home page, repeated as structured data. Written into
// the page between two markers, so what a crawler reads is what a visitor
// reads.
let index = read("src/index.html");
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
// Alpine and its persist plugin, as one file the site serves itself. The
// plugin comes first: it has to be there when Alpine starts.
write("public/assets/alpine.js", `${read("node_modules/@alpinejs/persist/dist/cdn.min.js")}\n${read("node_modules/alpinejs/dist/cdn.min.js")}\n`);

// The sheet, the scripts and the sprite get their contents' hash on their
// address, so a browser never pairs a new page with one it kept from before.
const stamped = (page) =>
  page.replace(/assets\/(?:site\.css|site\.js|alpine\.js)|img\/icons\.svg/g, (path) => {
    const hash = createHash("sha256").update(readFileSync(new URL(`public/${path}`, here)));
    return `${path}?v=${hash.digest("hex").slice(0, 8)}`;
  });
index = stamped(index);
write("public/index.html", index);

// The home page's header and footer, with addresses that hold at any depth.
const rooted = (part) =>
  part
    .replaceAll('href="./"', 'href="/"')
    .replaceAll('src="img/', 'src="/img/')
    .replaceAll('href="img/', 'href="/img/')
    .replaceAll('data-search="assets/', 'data-search="/assets/')
    .replaceAll('href="#', 'href="/#')
    .replaceAll('href="changelog/"', 'href="/changelog/"')
    .replaceAll('href="licence/"', 'href="/licence/"')
    .replaceAll('href="privacy/"', 'href="/privacy/"')
    .replaceAll('href="docs/"', 'href="/docs/"');
const header = rooted(index.match(/<header[\s\S]*?<\/header>/)[0]);
const footer = rooted(index.match(/<footer[\s\S]*?<\/footer>/)[0]);

function shell({ path, title, description }, main) {
  return stamped(`<!doctype html>
<html lang="en" class="scroll-smooth antialiased motion-reduce:scroll-auto">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <meta name="color-scheme" content="light dark" />
    <title>${plain(title)} | Clauset</title>
    <meta name="description" content="${plain(description)}" />
    <link rel="canonical" href="${site}/${path}" />
    <link rel="alternate" type="text/markdown" href="${site}/${path}index.md" />
    <meta property="og:site_name" content="Clauset" />
    <meta property="og:type" content="article" />
    <meta property="og:url" content="${site}/${path}" />
    <meta property="og:locale" content="en_GB" />
    <meta property="og:title" content="${plain(title)} | Clauset" />
    <meta property="og:description" content="${plain(description)}" />
    <meta property="og:image" content="${site}/img/og.png" />
    <meta property="og:image:width" content="1200" />
    <meta property="og:image:height" content="630" />
    <meta property="og:image:alt" content="Clauset: every Claude Code account, in your menu bar" />
    <meta name="twitter:card" content="summary_large_image" />
    <meta name="twitter:image" content="${site}/img/og.png" />
    <meta name="theme-color" content="#f4f5fa" media="(prefers-color-scheme: light)" />
    <meta name="theme-color" content="#0d1017" media="(prefers-color-scheme: dark)" />
    <link rel="apple-touch-icon" href="/img/apple-touch-icon.png" />
    <link rel="icon" href="/img/favicon.png" />
    <link rel="stylesheet" href="/assets/site.css" />
    <script>
      document.documentElement.classList.add("js");
    </script>
    <script src="https://cdn.visitors.now/v.js" data-token="4f0d95e5-7b3a-4e5a-a17f-5700e4ee8e59"></script>
    <script src="/assets/site.js" defer></script>
    <script src="/assets/alpine.js" defer></script>
  </head>
  <body class="bg-page text-ink dark:bg-night dark:text-fog">
    <div class="isolate" x-data>
      ${header}
      ${main}
      ${footer}
    </div>
  </body>
</html>
`);
}

const entry =
  "block rounded-lg px-3 py-2 text-base/6 text-muted hover:text-ink aria-[current=page]:bg-ink/5 aria-[current=page]:text-ink sm:py-1.5 sm:text-sm/6 dark:text-mist dark:hover:text-fog dark:aria-[current=page]:bg-white/8 dark:aria-[current=page]:text-fog";
function beside(page, { label, menu, body }) {
  return shell(
    page,
    `<main class="py-10 sm:py-16">
        <div class="mx-auto max-w-6xl px-6 lg:grid lg:grid-cols-[15rem_minmax(0,1fr)] lg:gap-x-16 lg:px-8">
          <details class="group surface mb-10 rounded-2xl lg:hidden">
            <summary class="flex cursor-pointer list-none items-center justify-between px-4 py-3 text-base/6 font-medium [&::-webkit-details-marker]:hidden">
              ${label}
              <svg class="size-4 shrink-0 fill-current transition-transform duration-150 group-open:rotate-180" aria-hidden="true"><use href="/img/icons.svg#caret-down" /></svg>
            </summary>
            <nav aria-label="${label}" class="space-y-6 px-1 pb-3">${menu}</nav>
          </details>
          <nav aria-label="${label}" class="sticky top-20 -ml-3 hidden max-h-[calc(100dvh-6rem)] space-y-8 self-start overflow-y-auto pb-8 [scrollbar-width:none] lg:block [&::-webkit-scrollbar]:hidden">${menu}</nav>
          <div class="min-w-0">${body}</div>
        </div>
      </main>`,
  );
}
const heading = (title, lede, crumb = "") => `${crumb}
            <h1 class="max-w-[24ch] text-3xl font-semibold tracking-tight text-pretty sm:text-4xl lg:text-5xl">${inline(title)}</h1>
            <p class="mt-4 max-w-[60ch] text-base/7 text-pretty text-muted sm:mt-5 sm:text-lg/8 dark:text-mist">${inline(lede)}</p>`;

const menu = (current) =>
  categories
    .map(
      (category) => `<div>
              <p class="px-3 text-base/6 font-semibold sm:text-sm/6">${category.title}</p>
              <ul class="mt-2 space-y-0.5">${category.articles
                .map((article) => {
                  const sections =
                    article === current && article.sections.length
                      ? `<ul class="mt-1 mb-2 ml-3 border-l border-ink/10 dark:border-white/10">${article.sections
                          .map(
                            (section) =>
                              `<li><a href="#${section.id}" class="block py-1.5 pl-4 text-base/6 text-muted hover:text-ink sm:py-1 sm:text-sm/6 dark:text-mist dark:hover:text-fog">${inline(section.text)}</a></li>`,
                          )
                          .join("")}</ul>`
                      : "";
                  return `<li><a href="${article.url}"${article === current ? ' aria-current="page"' : ""} class="${entry}">${inline(article.title)}</a>${sections}</li>`;
                })
                .join("")}</ul>
            </div>`,
    )
    .join("");

const docs = {
  path: "docs/",
  title: "Documentation",
  description: "How Clauset picks an account, what the accounts share, how a session moves and what Clauset writes on your Mac.",
};
const releases = {
  path: "changelog/",
  title: "Changelog",
  description: "What changed in each version of Clauset, the menu bar app and the CLI, newest first.",
};

const markdown = {};
const pages = [
  {
    path: "",
    title: home.querySelector("title").text,
    description: home.querySelector('meta[name="description"]').getAttribute("content"),
  },
  docs,
];

const card = (article) => `<a href="${article.url}" class="card block p-6">
                <p class="text-base/7 font-semibold">${inline(article.title)}</p>
                <p class="mt-1 text-base/6 text-pretty text-muted sm:text-sm/6 dark:text-mist">${inline(article.description)}</p>
              </a>`;
write(
  "public/docs/index.html",
  beside(docs, {
    label: "Documentation",
    menu: menu(),
    body: `${heading(docs.title, docs.description)}${categories
      .map(
        (category) => `
            <section id="${category.slug}" class="mt-14 scroll-mt-24">
              <h2 class="text-2xl font-semibold tracking-tight">${category.title}</h2>
              <p class="mt-2 max-w-[60ch] text-base/7 text-pretty text-muted dark:text-mist">${inline(category.description)}</p>
              <div class="mt-6 grid grid-cols-1 gap-4 sm:grid-cols-2">
              ${category.articles.map(card).join("\n              ")}
              </div>
            </section>`,
      )
      .join("")}`,
  }),
);
markdown[docs.path] = `# ${docs.title}\n\n${docs.description}\n\n${categories
  .map(
    (category) =>
      `## ${category.title}\n\n${category.description}\n\n${category.articles
        .map((article) => `- [${article.title}](${site}${article.url}index.md): ${article.description}`)
        .join("\n")}`,
  )
  .join("\n\n")}\n`;

const turn = (article, word) =>
  article
    ? `<a href="${article.url}" class="${word === "Next" ? "sm:col-start-2 sm:text-right " : ""}card block p-5">
                <span class="block text-base/6 text-muted sm:text-sm/6 dark:text-mist">${word}</span>
                <span class="mt-1 block text-base/7 font-semibold">${inline(article.title)}</span>
              </a>`
    : "";
articles.forEach((article, at) => {
  const page = { path: article.url.slice(1), title: article.title, description: article.description };
  pages.push(page);
  write(
    `public/${page.path}index.html`,
    beside(page, {
      label: "Documentation",
      menu: menu(article),
      body: `${heading(
        article.title,
        article.description,
        `<p class="mb-4 text-base/6 text-muted sm:text-sm/6 dark:text-mist"><a href="/docs/" class="hover:text-ink dark:hover:text-fog">Documentation</a> <span aria-hidden="true" class="px-1">/</span> ${article.category}</p>`,
      )}
            <article class="prose mt-10">
${html(article.body, article.path)}
            </article>
            <nav aria-label="More articles" class="mt-16 grid max-w-[72ch] grid-cols-1 gap-4 sm:grid-cols-2">
              ${turn(articles[at - 1], "Previous")}
              ${turn(articles[at + 1], "Next")}
            </nav>`,
    }),
  );
  markdown[page.path] = `# ${article.title}\n\n${article.description}\n\n${absolute(article.body, article.path)}\n`;
});

pages.push(releases);
write(
  "public/changelog/index.html",
  beside(releases, {
    label: "Versions",
    menu: `<ul class="space-y-0.5">${versions
      .map(
        (version) =>
          `<li><a href="#${version.id}" class="${entry}">${version.name}${version.date ? `<span class="block text-base/6 sm:text-xs/5">${day(version.date)}</span>` : ""}</a></li>`,
      )
      .join("")}</ul>`,
    body: `${heading(releases.title, releases.description)}
            <article class="prose mt-2">
${html(changelog, "CHANGELOG.md").replace(/<h2 id="[^"]*">\[([^\]]+)\](?: - (\d{4}-\d{2}-\d{2}))?<\/h2>/g, (_, name, date) => {
  const version = versions.find((entry) => entry.name === name);
  return `<h2 id="${version.id}">${name}${date ? ` <time datetime="${date}">${day(date)}</time>` : ""}</h2>`;
})}
            </article>`,
  }),
);
markdown[releases.path] = absolute(read("../CHANGELOG.md"), "CHANGELOG.md");

const licence = { path: "licence/", title: "Licence", description: "Clauset is free and open source under the MIT licence." };
const terms = read("../LICENSE")
  .replace(/^MIT License\n+/, "")
  .replace("(c)", "©")
  .trim();
const credits = read("../README.md")
  .match(/^## Credits\n+([\s\S]*?)(?=^## )/m)[1]
  .trim();
pages.push(licence);
write(
  "public/licence/index.html",
  shell(
    licence,
    `<main class="py-10 sm:py-16">
        <div class="mx-auto max-w-6xl px-6 lg:px-8">${heading(licence.title, licence.description)}
            <article class="prose mt-10">
${html(terms, "LICENSE")}
<h2 id="credits">Credits</h2>
${html(credits, "README.md")}
            </article>
        </div>
      </main>`,
  ),
);
markdown[licence.path] = `# ${licence.title}\n\n${licence.description}\n\n${terms}\n\n## Credits\n\n${absolute(credits, "README.md")}\n`;

const [, statement, policy] = read("../PRIVACY.md").match(/^# .*\n+(.*)\n+([\s\S]*)$/);
const privacy = { path: "privacy/", title: "Privacy", description: statement };
pages.push(privacy);
write(
  "public/privacy/index.html",
  shell(
    privacy,
    `<main class="py-10 sm:py-16">
        <div class="mx-auto max-w-6xl px-6 lg:px-8">${heading(privacy.title, privacy.description)}
            <article class="prose mt-10">
${html(policy, "PRIVACY.md")}
            </article>
        </div>
      </main>`,
  ),
);
markdown[privacy.path] = absolute(read("../PRIVACY.md"), "PRIVACY.md");

const outside = (href) => (/^(https?:|mailto:)/.test(href) ? href : `${site}/${href.replace(/^\.\//, "")}`);

function markdownOfHome() {
  // The parser keeps a `pre` as raw text unless told otherwise, markup and all.
  const main = parse(index, { blockTextElements: { script: true, style: true } }).querySelector("main");
  main.querySelectorAll('[aria-hidden="true"], picture, img, svg').forEach((node) => node.remove());
  const words = (node) =>
    node.childNodes
      .map((child) => {
        if (child.nodeType === 3) return child.text;
        const tag = child.rawTagName?.toLowerCase();
        if (tag === "code") return `\`${child.text.trim()}\``;
        if (tag === "a") return `[${words(child)}](${outside(child.getAttribute("href"))})`;
        return words(child);
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
      if (!node.closest("p, dd, dt, h1, h2, pre")) actions.push(`[${words(node)}](${outside(node.getAttribute("href"))})`);
      continue;
    }
    flush();
    if (tag === "h1") blocks.push(`# ${words(node)}`);
    else if (tag === "h2") blocks.push(`## ${words(node)}`);
    else if (tag === "dt") blocks.push(`### ${words(node)}`);
    else if (tag === "pre") blocks.push(`\`\`\`\n${node.text.replace(/\s+$/, "")}\n\`\`\``);
    else blocks.push(words(node));
  }
  flush();
  return `${blocks.join("\n\n")}\n`;
}
markdown[""] = markdownOfHome();

for (const page of pages) write(`public/${page.path}index.md`, markdown[page.path]);

write(
  "public/assets/search.json",
  `${JSON.stringify(
    pages.slice(1).map((page) => ({ title: page.title, description: page.description.replaceAll("`", ""), url: `/${page.path}` })),
  )}\n`,
);

const summary =
  "Clauset is a free, open-source menu bar app and CLI for macOS. It runs several Claude Code accounts on one Mac, sharing one ~/.claude, shows each account's usage limits, and lets you move a running session to another account when a limit nears. It never switches accounts by itself.";
const listed = (entries) => entries.map((page) => `- [${page.title}](${site}/${page.path}index.md): ${page.description}`).join("\n");
const llms = `# Clauset

> ${summary}

## Pages

${listed([pages[0], releases, licence, privacy])}

## Documentation

${listed(pages.filter((page) => page.path.startsWith("docs/")))}

## Source

- [GitHub repository](${repository}): the app, the CLI and this site, under the MIT licence.
`;
write("public/llms.txt", llms);
write("public/llms-full.txt", `${llms}\n${pages.map((page) => `---\n\nSource: ${site}/${page.path}\n\n${markdown[page.path]}`).join("\n")}`);

write("public/robots.txt", `User-agent: *\nAllow: /\n\nSitemap: ${site}/sitemap.xml\n`);
write(
  "public/sitemap.xml",
  `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${pages.map((page) => `  <url>\n    <loc>${site}/${page.path}</loc>\n  </url>`).join("\n")}
</urlset>
`,
);
