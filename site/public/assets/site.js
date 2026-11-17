// What moves on the page. Reveals and the first fill of the meters play
// once. The pictures in the feature grid loop, each only while it is in
// view and the tab shows, and not at all for someone who asked for less
// motion: they then see each picture's end state.
(() => {
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

  // Notifications arrive one by one, the newest in front, then clear.
  function notifications(stack) {
    const notes = [...stack.children];
    const place = (count) => {
      notes.forEach((note, index) => {
        note.toggleAttribute("data-waiting", index >= count);
        note.style.setProperty("--at", Math.max(0, count - 1 - index));
      });
    };
    if (still) return place(notes.length);
    place(0);
    loop(stack, async () => {
      await wait(500);
      for (let count = 1; count <= notes.length; count++) {
        place(count);
        await wait(1300);
      }
      await wait(2600);
      stack.setAttribute("data-leaving", "");
      await wait(350);
      place(0);
      stack.removeAttribute("data-leaving");
      await wait(500);
    });
  }

  // The sessions of one account go to the other: each is put in its new
  // place, drawn where it was, and let go. Then the picture starts over.
  function sessions(root) {
    const from = root.querySelector("[data-from]");
    const to = root.querySelector("[data-to]");
    const moving = [...from.children];
    const chips = [...root.querySelectorAll(".chip")];
    if (still) return moving.forEach((chip) => to.append(chip));
    loop(root, async () => {
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
      root.style.opacity = "0";
      await wait(250);
      moving.forEach((chip) => from.append(chip));
      root.style.opacity = "";
      await wait(400);
    });
  }

  // Usage creeps up, one limit at a time, and a window that fills resets.
  function usage(root) {
    if (still) return;
    const gauges = [...root.querySelectorAll(".gauge")];
    loop(root, async () => {
      await wait(1800);
      const gauge = gauges[Math.floor(Math.random() * gauges.length)];
      const bar = gauge.querySelector("b");
      const label = gauge.lastElementChild;
      const percent = parseInt(label.textContent, 10) + 1 + Math.floor(Math.random() * 3);
      const next = percent > 97 ? 3 : percent;
      bar.style.transitionDelay = "0ms";
      bar.style.setProperty("--p", `${next}%`);
      bar.classList.toggle("warn", next >= 70 && next < 90);
      bar.classList.toggle("full", next >= 90);
      label.textContent = `${next}%`;
    });
  }

  // The default account goes back and forth, and `claude` follows it.
  function chosen(root) {
    if (still) return;
    const rows = [...root.querySelectorAll("[data-account]")];
    const says = root.querySelector("[data-says]");
    let at = rows.findIndex((row) => row.hasAttribute("data-chosen"));
    loop(root, async () => {
      await wait(3200);
      at = (at + 1) % rows.length;
      rows.forEach((row, index) => {
        row.toggleAttribute("data-chosen", index === at);
        row.firstElementChild.textContent = index === at ? "✓" : "";
      });
      says.textContent = rows[at].dataset.account;
    });
  }

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
  document.querySelectorAll("[data-watch]").forEach((element) => seen.observe(element));

  document.querySelectorAll("[data-stack]").forEach(notifications);
  document.querySelectorAll("[data-move]").forEach(sessions);
  document.querySelectorAll("[data-usage]").forEach(usage);
  document.querySelectorAll("[data-default]").forEach(chosen);
})();
