// The markup holds every value a component starts from, so the page reads
// the same before Alpine runs. With reduced motion, each picture shows its
// end state.
document.addEventListener("alpine:init", () => {
  const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const wait = (ms) => new Promise((done) => setTimeout(done, ms));

  // Runs `cycle` again and again while `element` is on screen. A cycle
  // that has started plays to its end, so nothing stops half-way.
  function loop(element, cycle) {
    let visible = false;
    let running = false;
    const run = async () => {
      if (running) return;
      running = true;
      while (visible && !document.hidden) await cycle();
      running = false;
    };
    new IntersectionObserver(
      ([entry]) => {
        visible = entry.isIntersecting;
        if (visible) run();
      },
      { threshold: 0.3 },
    ).observe(element);
    document.addEventListener("visibilitychange", () => visible && run());
  }

  Alpine.store("accounts", {
    used: { Personal: 18, Work: 74 },
    week: { Personal: 28, Work: 61 },
    current: "Personal",
    taken: false,
    get level() {
      return this.used[this.current];
    },
    get other() {
      return this.current === "Personal" ? "Work" : "Personal";
    },
    pick(name) {
      this.taken = true;
      this.current = name;
    },
    shown: 18,
  });
  let counting;
  Alpine.effect(() => {
    const accounts = Alpine.store("accounts");
    const to = accounts.level;
    // Read raw: a tracked read would run this effect again on every frame
    // that `shown` counts.
    const from = Alpine.raw(accounts).shown;
    const began = (counting = performance.now());
    if (still || from === to) return (accounts.shown = to);
    const step = (now) => {
      if (counting !== began) return;
      const done = Math.min((now - began) / 700, 1);
      accounts.shown = Math.round(from + (to - from) * (1 - (1 - done) ** 3));
      if (done < 1) requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  });

  Alpine.store("clock", {
    now: "",
    init() {
      this.tell();
      setInterval(() => this.tell(), 15000);
    },
    tell() {
      const now = new Date();
      const part = (options) => now.toLocaleDateString("en-GB", options);
      const time = `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;
      this.now = `${part({ weekday: "short" })} ${now.getDate()} ${part({ month: "short" })} ${time}`;
    },
  });

  // x-meter: a bar shows a percentage, blue, orange from 70 and red from
  // 90, as the app colours a limit.
  Alpine.directive("meter", (el, { expression }, { evaluateLater, effect }) => {
    const percent = evaluateLater(expression);
    effect(() =>
      percent((value) => {
        el.style.setProperty("--p", `${value}%`);
        el.classList.toggle("warn", value >= 70 && value < 90);
        el.classList.toggle("full", value >= 90);
      }),
    );
  });

  const seen = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        seen.unobserve(entry.target);
        entry.target.setAttribute("data-seen", "");
      }
    },
    { threshold: 0.2 },
  );
  Alpine.directive("reveal", (el) => seen.observe(el));

  // Puts `text` on the clipboard. Where the clipboard API is missing or
  // refuses, as on a page a browser doesn't count as secure, a selection
  // and the old copy command do it.
  async function copy(text) {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch {
      const area = document.createElement("textarea");
      area.value = text;
      area.setAttribute("readonly", "");
      area.style.cssText = "position:fixed;opacity:0";
      document.body.append(area);
      area.select();
      const done = document.execCommand("copy");
      area.remove();
      return done;
    }
  }

  // A copy button copies its `data-copy`, says what happened, then goes
  // back to what it said. `said` outlasts `saying` by the length of the
  // fade, so the words don't vanish before they have faded.
  Alpine.data("copy", (idle = "Copy", done = "Copied") => ({
    said: "",
    saying: false,
    get label() {
      return this.saying ? this.said : idle;
    },
    async run() {
      this.said = (await copy(this.$root.dataset.copy)) ? done : "Failed";
      this.saying = true;
      clearTimeout(this.reset);
      this.reset = setTimeout(() => {
        this.saying = false;
        this.reset = setTimeout(() => (this.said = ""), 250);
      }, 1600);
    },
  }));

  Alpine.data("menubar", function () {
    return {
      names: ["clauset", "features", "download", "help", "app"],
      open: null,
      auto: false,
      // Whether the visitor left the app's own menu open or closed, so a
      // page that opens it by itself respects a closed one.
      memory: this.$persist(null).as("clauset.app-menu"),
      query: "",
      pages: null,

      // On the home page, with room for it, the app's own menu drops open
      // by itself: the hero points at it. It closes as any menu does, and
      // when the page scrolls away from the hero, which isn't remembered.
      init() {
        if (!document.querySelector("[data-opens-menu]") || !matchMedia("(min-width: 80rem)").matches || this.memory === "closed") return;
        setTimeout(() => {
          if (this.open || scrollY > 40) return;
          this.show("app");
          this.auto = true;
          const away = () => {
            if (scrollY < 200) return;
            removeEventListener("scroll", away);
            if (this.open === "app" && this.auto) this.hide();
            this.auto = false;
          };
          addEventListener("scroll", away, { passive: true });
        }, 700);
      },

      title(name) {
        return {
          ":aria-expanded"() {
            return this.open === name;
          },
          "@click"() {
            this.toggle(name);
          },
          "@pointerenter"(event) {
            if (this.open && this.open !== name && event.pointerType === "mouse") this.show(name);
          },
        };
      },
      drop(name) {
        return {
          ":hidden"() {
            return this.open !== name;
          },
          ":data-auto"() {
            return name === "app" && this.auto;
          },
          "@click"(event) {
            if (event.target.closest("a")) this.hide();
          },
        };
      },

      show(name) {
        if (this.open === name) return;
        this.auto = false;
        this.open = name;
        // A menu starts under its title's left edge. One that would run off
        // the screen there ends at its title's right edge instead, as on a
        // Mac, and no further left than the screen. The page's own width is
        // the measure: a phone's browser widens `innerWidth` to fit a menu
        // that runs off screen. A microtask runs after Alpine has shown the
        // menu and before the browser paints it.
        queueMicrotask(() => {
          const menu = this.$refs[name];
          const drop = menu.lastElementChild;
          const title = menu.firstElementChild.getBoundingClientRect();
          const fits = title.left + drop.offsetWidth <= document.documentElement.clientWidth - 6;
          drop.style.left = fits ? "0px" : `${Math.max(6 - title.left, title.width - drop.offsetWidth)}px`;
        });
      },
      hide() {
        this.open = null;
        this.auto = false;
      },
      toggle(name) {
        if (this.open === name) this.hide();
        else this.show(name);
        if (name === "app") this.memory = this.open === "app" ? "open" : "closed";
      },
      away() {
        if (this.open === "app") this.memory = "closed";
        this.hide();
      },

      // A menu that opened by itself leaves the keys to the page, so an
      // arrow still scrolls it, until the visitor's focus is inside it.
      // Escape closes it all the same.
      key(event) {
        if (!this.open) return;
        const menu = this.$refs[this.open];
        if (this.auto && event.key !== "Escape" && !menu.contains(document.activeElement)) return;
        if (event.key === "Escape") {
          this.away();
          menu.firstElementChild.focus();
        } else if ((event.key === "ArrowRight" || event.key === "ArrowLeft") && !event.target.matches("input")) {
          const shown = this.names.filter((name) => this.$refs[name].offsetParent);
          const next = shown[(shown.indexOf(this.open) + (event.key === "ArrowRight" ? 1 : shown.length - 1)) % shown.length];
          this.show(next);
          this.$refs[next].firstElementChild.focus();
        } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
          event.preventDefault();
          const items = [...menu.lastElementChild.querySelectorAll("a, button, input")].filter((item) => item.offsetParent);
          const at = items.indexOf(document.activeElement);
          const step = event.key === "ArrowDown" ? 1 : at === -1 ? 0 : items.length - 1;
          items[(at + step + items.length) % items.length].focus();
        }
      },

      async load() {
        this.pages = await fetch(this.$refs.search.dataset.search).then((reply) => reply.json());
      },
      get hits() {
        const terms = this.query.toLowerCase().split(/\s+/).filter(Boolean);
        if (!terms.length || !this.pages) return [];
        return this.pages.filter((page) => terms.every((term) => `${page.title} ${page.description}`.toLowerCase().includes(term))).slice(0, 6);
      },
    };
  });

  Alpine.data("live", () => ({
    open: false,
    pressed: false,
    marked: "Personal",

    init() {
      this.$watch("$store.accounts.current", (name) => this.swap(name));
      if (!still) loop(this.$el, () => this.play());
    },

    // The word in place is the last child: during a swap the one before it
    // is still leaving.
    swap(word) {
      const slot = this.$refs.word;
      const old = slot.lastElementChild;
      const next = document.createElement("span");
      next.textContent = word;
      next.setAttribute("data-arriving", "");
      slot.append(next);
      old.setAttribute("data-leaving", "");
      requestAnimationFrame(() => requestAnimationFrame(() => next.removeAttribute("data-arriving")));
      setTimeout(() => old.remove(), 420);
    },

    toggle() {
      this.$store.accounts.taken = true;
      this.marked = this.$store.accounts.current;
      this.open = !this.open;
    },
    choose(name) {
      this.open = false;
      this.$store.accounts.pick(name);
    },

    // The account in use fills up into the red. An account left in the red
    // has had its window reset by the time the other one is full.
    async play() {
      const accounts = this.$store.accounts;
      await wait(2400);
      if (accounts.taken) return;
      const other = accounts.other;
      accounts.used[accounts.current] = 92;
      if (accounts.used[other] >= 90) accounts.used[other] = 18;
      await wait(2000);
      if (accounts.taken) return;
      this.pressed = true;
      await wait(140);
      this.pressed = false;
      this.marked = accounts.current;
      this.open = true;
      await wait(800);
      if (accounts.taken) return;
      this.marked = other;
      await wait(450);
      if (accounts.taken) return;
      this.open = false;
      accounts.current = other;
      await wait(600);
    },
  }));

  Alpine.data("stack", () => ({
    count: 0,
    leaving: false,
    init() {
      const total = this.$el.children.length;
      if (still) return (this.count = total);
      loop(this.$el, async () => {
        await wait(500);
        for (let count = 1; count <= total; count++) {
          this.count = count;
          await wait(1300);
        }
        await wait(2600);
        this.leaving = true;
        await wait(350);
        this.count = 0;
        this.leaving = false;
        await wait(500);
      });
    },
    note(index) {
      return {
        ":data-waiting"() {
          return index >= this.count;
        },
        ":style"() {
          return { "--at": Math.max(0, this.count - 1 - index) };
        },
      };
    },
  }));

  // Each session is put in its new place, drawn where it was, and let go.
  Alpine.data("move", () => ({
    faded: false,
    init() {
      const { from, to } = this.$refs;
      const moving = [...from.children];
      const chips = [...this.$el.querySelectorAll(".chip")];
      if (still) return moving.forEach((chip) => to.append(chip));
      loop(this.$el, async () => {
        await wait(1400);
        const before = new Map(chips.map((chip) => [chip, chip.getBoundingClientRect()]));
        moving.forEach((chip) => to.append(chip));
        chips.forEach((chip, index) => {
          const was = before.get(chip);
          const now = chip.getBoundingClientRect();
          const dx = was.left - now.left;
          const dy = was.top - now.top;
          if (!dx && !dy) return;
          chip.style.transition = "none";
          chip.style.transform = `translate(${dx}px, ${dy}px)`;
          chip.getBoundingClientRect();
          chip.style.transition = "";
          chip.style.transitionDelay = `${index * 80}ms`;
          chip.style.transform = "";
        });
        await wait(3600);
        this.faded = true;
        await wait(250);
        moving.forEach((chip) => from.append(chip));
        this.faded = false;
        await wait(400);
      });
    },
  }));

  Alpine.data("usage", (values) => ({
    values,
    init() {
      if (still) return;
      loop(this.$el, async () => {
        await wait(1800);
        const at = Math.floor(Math.random() * this.values.length);
        const next = this.values[at] + 1 + Math.floor(Math.random() * 3);
        this.values[at] = next > 97 ? 3 : next;
      });
    },
  }));

  Alpine.data("chosen", (names, at) => ({
    at,
    get name() {
      return names[this.at];
    },
    init() {
      if (still) return;
      loop(this.$el, async () => {
        await wait(3200);
        this.at = (this.at + 1) % names.length;
      });
    },
  }));
});
