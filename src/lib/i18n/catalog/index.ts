/**
 * Area catalogs. Each module exports `en` (source) and one object per
 * translation with the same keys. Keys start with the area's prefixes, so
 * merged catalogs never collide (a test checks it).
 */
import * as shell from "./shell";
import * as app from "./app";
import * as branches from "./branches";
import * as config from "./config";
import * as diff from "./diff";
import * as github from "./github";
import * as graph from "./graph";
import * as operations from "./operations";
import * as settings from "./settings";
import * as staging from "./staging";

export const AREAS = { shell, app, branches, config, diff, github, graph, operations, settings, staging };
