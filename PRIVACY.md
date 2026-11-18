# Privacy

What Clauset and clauset.app do with data.

## The CLI and the app

`clauset` makes no network call. It reads and writes its own config. It links and moves the files Claude Code keeps per account, and never reads a sign-in or sends one anywhere. Nothing about your accounts, your usage or your sessions leaves your Mac through Clauset.

The menu bar app asks before it goes online. With your yes it fetches its release feed from GitHub once a week, and it downloads a newer version from there when you choose to install it. GitHub sees your IP address then, as it does with any download. The request carries the app's name and version, and nothing about you or your accounts. You can turn the weekly check off in the app's settings.

## This website

clauset.app counts visits with [Visitors](https://visitors.now). It sets no cookies. For each page you open, Visitors receives:

- the address of the page and the page you came from;
- your browser and device type, from the User-Agent your browser sends;
- your IP address, which it uses once to look up a city, region and country and then discards. An address that sends too many requests is kept for a short time to block abuse;
- how long the page stays open, which links to other sites you follow, and how fast the page loaded.

To count a visitor once a day, Visitors turns the IP address, the User-Agent and a salt that changes every midnight into a hash, so a visitor gets a different one each day.

We use these numbers to see how many people visit, which pages they read and how fast the site is. The legal ground is our legitimate interest in that (article 6(1)(f) of the GDPR).

Visitors Now, LLC, a company in Delaware in the United States, runs the service for us under a data processing agreement. The agreement says Visitors uses the data for nothing of its own, and that the visit data is processed and stored in the European Union, at Hetzner in Germany. Cloudflare delivers the script, so it sees your IP address when the script loads. Visitors keeps the visit data for as long as our account with it exists, and deletes the speed measurements after one year.

## The web server

Our web server logs each request: IP address, time, page, referrer and User-Agent. We use the log to keep the site running and to spot abuse (legitimate interest, article 6(1)(f) of the GDPR). A log is deleted within eight weeks. DigitalOcean hosts the server in Amsterdam, in the Netherlands.

Cloudflare and DigitalOcean are American companies. Both are certified under the EU-US Data Privacy Framework.

## Your right to object

You can object to the counting and to the server log at any time. For the counting, turn on Global Privacy Control or Do Not Track in your browser and the script sends nothing. For the log, write to us at the address below.

## Your other rights

You can ask us for access to your data, to have it corrected or erased, or to have its use restricted. We can't find your visits in the counting: Visitors doesn't keep your IP address, and the hash changes every day. The server log is different. Tell us your IP address and the time of your visit, and we'll look.

You can also complain to the Dutch data protection authority, the [Autoriteit Persoonsgegevens](https://autoriteitpersoonsgegevens.nl).

## Who is responsible

Hold My Beer B.V., Keizersgracht 520H, 1017 EK Amsterdam, the Netherlands, is the controller for clauset.app. KvK number 75807408.

Write to [privacy@clauset.app](mailto:privacy@clauset.app) about anything to do with your data. The issue tracker is public, so don't use it for that.
