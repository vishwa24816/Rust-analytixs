import { render as topstats } from './topstats.js';
import { draw as graph } from './graph.js';
import { table } from './breakdown.js';
import { render as filters } from './filters.js';
import { render as periods } from './period.js';
import { start as realtime } from './realtime.js';
import { render as map } from './map.js';
import { render as annotations } from './annotations.js';
import { render as segments } from './segments.js';
import { render as funnel } from './funnel.js';
import { render as journey } from './journey.js';
import { render as keywords } from './keywords.js';
import { render as props } from './props.js';
import { render as conversions } from './conversions.js';
import { render as sources } from './sources.js';

async function refresh() {
  await topstats();
  await graph();
  await Promise.all([
    table('bd-pages', 'event:page', 'pageviews', 10, 'Top pages'),
    sources(),
    table('bd-browsers', 'visit:browser', 'visitors', 10, 'Browsers'),
    table('bd-devices', 'visit:device', 'visitors', 10, 'Devices'),
    table('bd-countries', 'visit:country', 'visitors', 10, 'Countries'),
    table('bd-entry', 'visit:entry_page', 'visitors', 10, 'Entry pages'),
    table('bd-exit', 'visit:exit_page', 'visitors', 10, 'Exit pages'),
    table('bd-events', 'event:name', 'visitors', 10, 'Custom events', ['pageview', 'engagement']),
    conversions(),
  ]);
  await map();
  await annotations();
  await funnel();
  await keywords();
}

filters();
segments();
journey();
props();
periods(refresh);
realtime();
refresh();
