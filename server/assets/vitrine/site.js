// qui de nous ? landing page: question ticker, playable demo round, screenshot rail, parallax, confetti.
// Vanilla JS, no dependencies. Text comes from the page language (<html lang>).
(function () {
  'use strict';
  var EN = document.documentElement.lang === 'en';
  var calm = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  var finePointer = window.matchMedia('(hover: hover) and (pointer: fine)').matches;

  var T = EN
    ? {
        questions: [
          'Who would get thrown out of a wedding?', 'Who would die first in a horror film?',
          'Who would survive longest in a zombie apocalypse?', 'Who would set the kitchen on fire making pasta?',
          'Who would make the worst spy?', 'Who would become an influencer if they could?',
          'Who is late even to their own birthday?', 'Who would end up on a reality TV show?',
          'Who would cheat to win a board game?', 'Who would go to Mars with no return ticket?',
          'Who would stay in pyjamas all Sunday?', 'Who would be the last one standing at a party?',
          'Who would start a podcast nobody asked for?', 'Who would get lost even with a GPS?'
        ],
        tag: 'Us · #',
        choose: 'Pick someone',
        voteFor: function (n) { return 'Vote for ' + n; },
        waiting: function (k) { return 'Waiting for the others… ' + k + '/7'; },
        drum: 'The verdict is in…',
        next: 'Next question',
        me: 'Y', mine: 'you',
        verdicts: ['Verdict: {n}. Own it.', "It's {n}. No appeal possible.", '{n} wins. The chat is going to heat up.'],
        tie: function (a) { return 'Tie: ' + a + '. Settle it in the chat.'; },
        and: ' and ',
        alone: function (n) { return 'Nobody else voted for ' + n + '.'; },
        withOthers: function (n, k) { return 'You voted for ' + n + ', like ' + k + (k > 1 ? ' others.' : ' other.'); },
        won: function (x) { return x > 0 ? '+' + x + ' chips' : 'Everyone bet the same way: refunded.'; },
        lost: '−10 chips',
        broke: '<b>Out of chips:</b> back to 100 on the 1st of the month',
        betLabel: '<b>Bet 10 chips</b> on my pick'
      }
    : {
        questions: [
          "Qui se ferait virer d'un mariage\u00a0?", "Qui mourrait en premier dans un film d'horreur\u00a0?",
          'Qui survivrait le plus longtemps à une apocalypse zombie\u00a0?', 'Qui ferait exploser la cuisine en faisant des pâtes\u00a0?',
          'Qui serait le pire espion\u00a0?', "Qui deviendrait influenceur s'il le pouvait\u00a0?",
          'Qui arrive en retard même à son propre anniversaire\u00a0?', 'Qui finirait dans une émission de télé-réalité\u00a0?',
          'Qui tricherait pour gagner un jeu de société\u00a0?', 'Qui irait sur Mars sans billet de retour\u00a0?',
          'Qui resterait en pyjama tout le dimanche\u00a0?', 'Qui serait le dernier debout à une soirée\u00a0?',
          "Qui lancerait un podcast que personne n'a demandé\u00a0?", 'Qui se perdrait même avec un GPS\u00a0?'
        ],
        tag: 'De nous · #',
        choose: "Choisis quelqu'un",
        voteFor: function (n) { return 'Voter pour ' + n; },
        waiting: function (k) { return 'Les autres votent… ' + k + '/7'; },
        drum: 'Le verdict tombe…',
        next: 'Question suivante',
        me: 'T', mine: 'toi',
        verdicts: ['Verdict : {n}. Assume.', "C'est {n}. Pas de recours possible.", '{n} l’emporte. Le chat va chauffer.'],
        tie: function (a) { return 'Égalité : ' + a + '. Ça se règle dans le chat.'; },
        and: ' et ',
        alone: function (n) { return "Personne d'autre n'a voté " + n + '.'; },
        withOthers: function (n, k) { return 'Tu as voté ' + n + ', comme ' + k + (k > 1 ? ' autres.' : ' autre.'); },
        won: function (x) { return x > 0 ? '+' + x + ' jetons' : 'Tout le monde a misé pareil : remboursé.'; },
        lost: '−10 jetons',
        broke: '<b>Plus de jetons :</b> retour à 100 le 1er du mois',
        betLabel: '<b>Miser 10 jetons</b> sur mon choix'
      };

  var CREW = [
    { k: 'l', n: 'Léa' }, { k: 'h', n: 'Hugo' }, { k: 'i', n: 'Inès' },
    { k: 'n', n: 'Nathan' }, { k: 'c', n: 'Camille' }, { k: 'j', n: 'Jules' }
  ];
  var COLORS = { l: '#FF7A45', h: '#5BC8FF', i: '#FF5FB0', n: '#FFB547', c: '#B48CFF', j: '#5BF0B0', me: '#C8FF3D' };

  function $(sel, root) { return (root || document).querySelector(sel); }
  function $$(sel, root) { return Array.prototype.slice.call((root || document).querySelectorAll(sel)); }
  function el(tag, cls, text) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text != null) e.textContent = text;
    return e;
  }
  function avatar(k, letter) {
    var a = el('span', 'av av-' + k, letter);
    a.setAttribute('aria-hidden', 'true');
    return a;
  }
  function shuffle(a) {
    for (var i = a.length - 1; i > 0; i--) { var j = Math.floor(Math.random() * (i + 1)); var t = a[i]; a[i] = a[j]; a[j] = t; }
    return a;
  }
  function svgIcon(id) {
    var s = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    s.setAttribute('class', 'ic');
    s.setAttribute('aria-hidden', 'true');
    var u = document.createElementNS('http://www.w3.org/2000/svg', 'use');
    u.setAttribute('href', '#' + id);
    s.appendChild(u);
    return s;
  }

  // ---- Confetti ---------------------------------------------------------------------------------
  var canvas = $('[data-confetti]');
  var ctx = canvas && canvas.getContext && canvas.getContext('2d');
  var bits = [];
  var running = false;
  function burst(x, y, colors, count) {
    if (calm || !ctx) return;
    var dpr = Math.min(window.devicePixelRatio || 1, 2);
    canvas.width = innerWidth * dpr;
    canvas.height = innerHeight * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    for (var i = 0; i < (count || 110); i++) {
      var ang = Math.random() * Math.PI * 2;
      var sp = 4 + Math.random() * 9;
      bits.push({
        x: x, y: y, vx: Math.cos(ang) * sp, vy: Math.sin(ang) * sp - 6,
        w: 6 + Math.random() * 6, h: 4 + Math.random() * 6, r: Math.random() * 6, vr: (Math.random() - .5) * .4,
        c: colors[i % colors.length], life: 0, round: Math.random() < .3
      });
    }
    if (!running) { running = true; requestAnimationFrame(tick); }
  }
  function tick() {
    ctx.clearRect(0, 0, innerWidth, innerHeight);
    bits = bits.filter(function (b) { return b.life < 140 && b.y < innerHeight + 40; });
    bits.forEach(function (b) {
      b.life++; b.vy += .32; b.vx *= .985; b.x += b.vx; b.y += b.vy; b.r += b.vr;
      ctx.save();
      ctx.globalAlpha = Math.max(0, 1 - b.life / 140);
      ctx.translate(b.x, b.y); ctx.rotate(b.r); ctx.fillStyle = b.c;
      if (b.round) { ctx.beginPath(); ctx.arc(0, 0, b.h / 2, 0, Math.PI * 2); ctx.fill(); }
      else ctx.fillRect(-b.w / 2, -b.h / 2, b.w, b.h);
      ctx.restore();
    });
    if (bits.length) requestAnimationFrame(tick);
    else { running = false; ctx.clearRect(0, 0, innerWidth, innerHeight); }
  }
  function burstFrom(node, colors, count) {
    var r = node.getBoundingClientRect();
    burst(r.left + r.width / 2, r.top + r.height / 2, colors, count);
  }

  // ---- Top bar ----------------------------------------------------------------------------------
  var top = $('.top');
  function onScroll() { top.classList.toggle('scrolled', window.scrollY > 8); }
  window.addEventListener('scroll', onScroll, { passive: true });
  onScroll();

  // ---- Question ticker --------------------------------------------------------------------------
  var tickerQ = $('[data-ticker]');
  var tickerN = $('[data-ticker-n]');
  var deck = shuffle(T.questions.slice());
  var deckAt = 0;
  var n = 14;
  var paused = false;
  function nextQuestion() {
    var q = deck[deckAt++ % deck.length];
    if (q === tickerQ.textContent) q = deck[deckAt++ % deck.length];
    return q;
  }
  function rotate() {
    var q = nextQuestion();
    n++;
    if (calm) { tickerQ.textContent = q; tickerN.textContent = n; return; }
    tickerQ.classList.remove('in');
    tickerQ.classList.add('out');
    setTimeout(function () {
      tickerQ.textContent = q;
      tickerN.textContent = n;
      tickerQ.classList.remove('out');
      tickerQ.classList.add('in');
    }, 260);
  }
  var ticker = $('.ticker');
  ticker.addEventListener('pointerenter', function () { paused = true; });
  ticker.addEventListener('pointerleave', function () { paused = false; });
  ticker.addEventListener('focusin', function () { paused = true; });
  ticker.addEventListener('focusout', function () { paused = false; });
  $('[data-ticker-next]').addEventListener('click', rotate);
  if (!calm) setInterval(function () { if (!paused && !document.hidden) rotate(); }, 3800);

  var qmark = $('[data-burst]');
  qmark.addEventListener('click', function () {
    qmark.classList.remove('spin');
    void qmark.offsetWidth;
    qmark.classList.add('spin');
    burstFrom(qmark, ['#C8FF3D', '#FF6DB5', '#5BC8FF', '#FFB547', '#B48CFF', '#5BF0B0'], 90);
    rotate();
  });

  // ---- Hero parallax ----------------------------------------------------------------------------
  var vis = $('[data-parallax]');
  if (vis && finePointer && !calm) {
    var layers = $$('[data-depth]', vis);
    var hero = $('.hero');
    var raf = 0;
    hero.addEventListener('pointermove', function (e) {
      var r = hero.getBoundingClientRect();
      var px = (e.clientX - r.left) / r.width - .5;
      var py = (e.clientY - r.top) / r.height - .5;
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(function () {
        layers.forEach(function (l) {
          var d = +l.getAttribute('data-depth');
          l.style.setProperty('--tx', (px * d).toFixed(1) + 'px');
          l.style.setProperty('--ty', (py * d).toFixed(1) + 'px');
        });
      });
    });
    hero.addEventListener('pointerleave', function () {
      layers.forEach(function (l) { l.style.setProperty('--tx', '0px'); l.style.setProperty('--ty', '0px'); });
    });
  }

  // ---- The demo round ---------------------------------------------------------------------------
  var demo = $('[data-demo]');
  var grid = $('[data-demo-grid]', demo);
  var results = $('[data-demo-results]', demo);
  var go = $('[data-demo-go]', demo);
  var qEl = $('[data-demo-q]', demo);
  var tagEl = $('[data-demo-tag]', demo);
  var stack = $('[data-demo-stack]', demo);
  var count = $('[data-demo-count]', demo);
  var verdict = $('[data-demo-verdict]', demo);
  var walletEl = $('[data-demo-wallet]', demo);
  var bet = $('[data-demo-bet]', demo);
  var betBox = $('[data-demo-betbox]', demo);
  var betText = $('.bet-text', bet);
  var wallet = 100;
  var round = 1;
  var picked = null;
  var phase = 'vote';
  var qDeck = shuffle(T.questions.slice());

  go.removeAttribute('disabled');
  function setGo(text, enabled, icon) {
    go.textContent = '';
    if (icon) go.appendChild(svgIcon(icon));
    go.appendChild(document.createTextNode(text));
    go.setAttribute('aria-disabled', enabled ? 'false' : 'true');
  }

  function renderVote() {
    phase = 'vote';
    picked = null;
    qEl.textContent = qDeck[(round - 1) % qDeck.length];
    tagEl.textContent = T.tag + round;
    stack.textContent = '';
    count.textContent = '0/7';
    verdict.textContent = '';
    results.hidden = true;
    results.textContent = '';
    grid.hidden = false;
    grid.textContent = '';
    CREW.forEach(function (m) {
      var b = el('button', 'pick');
      b.type = 'button';
      b.setAttribute('aria-pressed', 'false');
      b.appendChild(avatar(m.k, m.n[0]));
      b.appendChild(document.createTextNode(m.n));
      b.addEventListener('click', function () {
        if (phase !== 'vote') return;
        picked = m;
        $$('.pick', grid).forEach(function (o) { o.setAttribute('aria-pressed', String(o === b)); });
        setGo(T.voteFor(m.n), true);
      });
      grid.appendChild(b);
    });
    var broke = wallet < 10;
    betBox.disabled = broke;
    if (broke) betBox.checked = false;
    betText.innerHTML = broke ? T.broke : T.betLabel;
    bet.classList.toggle('locked', broke);
    setGo(T.choose, false);
  }

  function weightedPick(weights) {
    var total = weights.reduce(function (a, b) { return a + b; }, 0);
    var r = Math.random() * total;
    for (var i = 0; i < weights.length; i++) { r -= weights[i]; if (r < 0) return i; }
    return weights.length - 1;
  }

  function play() {
    phase = 'waiting';
    $$('.pick', grid).forEach(function (b) { b.setAttribute('aria-disabled', 'true'); b.disabled = b.getAttribute('aria-pressed') !== 'true'; });
    betBox.disabled = true;
    bet.classList.add('locked');
    var betting = betBox.checked;
    // A question has its usual suspects: two favourites get most of the votes.
    var weights = CREW.map(function () { return 1; });
    var favs = shuffle([0, 1, 2, 3, 4, 5]);
    weights[favs[0]] = 6; weights[favs[1]] = 3.5;
    var votes = [{ from: 'me', to: CREW.indexOf(picked) }];
    CREW.forEach(function (m, i) { votes.push({ from: m.k, to: weightedPick(weights), friend: i }); });
    var order = [{ k: 'me', l: T.me }].concat(shuffle(CREW.map(function (m) { return { k: m.k, l: m.n[0] }; })));
    var step = calm ? 60 : 330;
    order.forEach(function (o, i) {
      setTimeout(function () {
        var a = avatar(o.k, o.l);
        stack.appendChild(a);
        count.textContent = (i + 1) + '/7';
        setGo(T.waiting(i + 1), false);
      }, i * step + (calm ? 0 : Math.random() * 120));
    });
    setTimeout(function () { setGo(T.drum, false); }, order.length * step + 150);
    setTimeout(function () { reveal(votes, betting); }, order.length * step + (calm ? 200 : 1000));
  }

  function reveal(votes, betting) {
    phase = 'result';
    var tally = CREW.map(function (m, i) { return { m: m, i: i, from: [] }; });
    votes.forEach(function (v) { tally[v.to].from.push(v.from); });
    var max = Math.max.apply(null, tally.map(function (t) { return t.from.length; }));
    var rows = tally.filter(function (t) { return t.from.length; }).sort(function (a, b) { return b.from.length - a.from.length; });
    var winners = rows.filter(function (t) { return t.from.length === max; });
    var mineIdx = CREW.indexOf(picked);
    grid.hidden = true;
    results.hidden = false;
    rows.forEach(function (t, idx) {
      var r = el('div', 'res' + (t.from.length === max ? ' win' : '') + (t.i === mineIdx ? ' mine' : ''));
      r.style.animationDelay = (calm ? 0 : idx * 90) + 'ms';
      var bar = el('span', 'res-bar');
      bar.style.background = COLORS[t.m.k];
      r.appendChild(bar);
      r.appendChild(avatar(t.m.k, t.m.n[0]));
      var name = el('span', 'name');
      if (t.i === mineIdx) name.setAttribute('data-mine', T.mine);
      name.appendChild(document.createTextNode(t.m.n));
      if (t.from.length === max) name.appendChild(svgIcon('i-crown'));
      r.appendChild(name);
      var vs = el('span', 'voters');
      t.from.slice(0, 4).forEach(function (k) { vs.appendChild(avatar(k, k === 'me' ? T.me : CREW.filter(function (m) { return m.k === k; })[0].n[0])); });
      r.appendChild(vs);
      var pct = Math.round((t.from.length / 7) * 100);
      r.appendChild(el('span', 'pct', pct + '%'));
      results.appendChild(r);
      requestAnimationFrame(function () { requestAnimationFrame(function () { bar.style.width = pct + '%'; }); });
    });

    // The Tripot: you (maybe) and three friends bet 10 chips each on their own vote; the pot goes to the right guesses.
    var bettors = shuffle(votes.slice(1)).slice(0, 3);
    if (betting) bettors.push(votes[0]);
    var pot = bettors.length * 10;
    var winIdx = winners.map(function (w) { return w.i; });
    var right = bettors.filter(function (b) { return winIdx.indexOf(b.to) >= 0; });
    var iWon = winIdx.indexOf(mineIdx) >= 0;
    var money = '';
    if (betting) {
      if (iWon) {
        var gain = Math.floor(pot / right.length);
        wallet += gain - 10;
        money = '<span class="gain">' + T.won(gain - 10) + '</span>';
      } else {
        wallet -= 10;
        money = '<span class="loss">' + T.lost + '</span>';
      }
      walletEl.textContent = wallet;
      var w = walletEl.parentNode;
      w.classList.remove('bump'); void w.offsetWidth; w.classList.add('bump');
    }

    var names = winners.map(function (w) { return w.m.n; });
    var head = names.length > 1
      ? T.tie(names.slice(0, -1).join(', ') + T.and + names[names.length - 1])
      : T.verdicts[Math.floor(Math.random() * T.verdicts.length)].replace('{n}', names[0]);
    var others = tally[mineIdx].from.length - 1;
    var line = others ? T.withOthers(picked.n, others) : T.alone(picked.n);
    verdict.innerHTML = '';
    verdict.appendChild(document.createTextNode(head));
    var small = el('small');
    small.appendChild(document.createTextNode(line + ' '));
    if (money) small.insertAdjacentHTML('beforeend', money);
    verdict.appendChild(small);
    setGo(T.next, true, 'i-redo');
    var winRow = $('.res.win', results);
    var colors = winners.map(function (w) { return COLORS[w.m.k]; }).concat(['#C8FF3D', '#FFFFFF']);
    if (winRow) setTimeout(function () { burstFrom(winRow, colors, iWon ? 150 : 70); }, calm ? 0 : 350);
  }

  go.addEventListener('click', function () {
    if (go.getAttribute('aria-disabled') === 'true') return;
    if (phase === 'vote' && picked) play();
    else if (phase === 'result') { round++; renderVote(); }
  });
  renderVote();

  // ---- Screenshot rail --------------------------------------------------------------------------
  var rail = $('[data-rail]');
  var slides = $$('.slide', rail);
  var chips = $$('[data-chips] button');
  var prev = $('[data-prev]');
  var next = $('[data-next]');
  function slideStep() { return slides[1].offsetLeft - slides[0].offsetLeft; }
  function current() { return Math.round(rail.scrollLeft / slideStep()); }
  function goTo(i) {
    i = Math.max(0, Math.min(slides.length - 1, i));
    rail.scrollTo({ left: slides[i].offsetLeft - slides[0].offsetLeft, behavior: calm ? 'auto' : 'smooth' });
  }
  function syncRail() {
    var max = rail.scrollWidth - rail.clientWidth - 4;
    var at = rail.scrollLeft >= max ? slides.length - 1 : current();
    chips.forEach(function (c, i) {
      c.setAttribute('aria-current', String(i === at));
    });
    prev.disabled = rail.scrollLeft < 4;
    next.disabled = rail.scrollLeft >= max;
  }
  chips.forEach(function (c) {
    c.addEventListener('click', function () {
      goTo(+c.getAttribute('data-go'));
      c.scrollIntoView({ block: 'nearest', inline: 'center', behavior: calm ? 'auto' : 'smooth' });
    });
  });
  prev.addEventListener('click', function () { goTo(current() - 1); });
  next.addEventListener('click', function () { goTo(current() + 1); });
  var syncRaf = 0;
  rail.addEventListener('scroll', function () { cancelAnimationFrame(syncRaf); syncRaf = requestAnimationFrame(syncRail); }, { passive: true });
  window.addEventListener('resize', syncRail);
  syncRail();

  // Tilt the phones under the pointer.
  if (finePointer && !calm) {
    $$('.tilt').forEach(function (p) {
      p.addEventListener('pointermove', function (e) {
        var r = p.getBoundingClientRect();
        var x = (e.clientX - r.left) / r.width - .5;
        var y = (e.clientY - r.top) / r.height - .5;
        p.style.transform = 'perspective(900px) rotateY(' + (x * 14).toFixed(1) + 'deg) rotateX(' + (-y * 12).toFixed(1) + 'deg) translateY(-6px)';
      });
      p.addEventListener('pointerleave', function () { p.style.transform = ''; });
    });
  }
})();
