"""WiFi location tracking.

Maintains two lists of access points on the context:
  wifi_familiar  - up to _FAMILIAR_MAX APs seen most often (persisted)
  wifi_recent    - up to _RECENT_MAX APs seen recently but not yet promoted (persisted)

Each entry is a dict:  {'b': 'aa:bb:cc:dd:ee:ff', 's': 'SSID', 'n': <float count>}

Scanning runs once at boot (clean memory) and on demand from the debug_wifi scene.
"""

import gc

_FAMILIAR_MAX   = 16
_RECENT_MAX     = 8
_PROMOTE_MIN    = 5.0   # min count before a recent entry can be promoted
_DECAY_PER_SCAN = 0.25  # subtracted from every unseen entry each scan


def _bssid_str(raw):
    return ':'.join('%02x' % b for b in raw)


def scan_now(context):
    """Perform a WiFi scan immediately.

    Updates context in-place and returns the raw AP list for callers that
    need RSSI / channel / auth data (e.g. the debug scene).  Returns [] on error.
    """
    try:
        import network
        import time
        wlan = network.WLAN(network.STA_IF)
        was_active = wlan.active()
        wlan.active(True)
        time.sleep_ms(200)
        gc.collect()
        aps = wlan.scan()
        if not was_active:
            wlan.active(False)
        _process(context, aps)
        print("[WiFi] Scan done. familiar=" + str(context.in_familiar_location) +
              " (" + str(len(context.wifi_familiar)) + "/" + str(_FAMILIAR_MAX) + " known)")
        return aps
    except Exception as e:
        print("[WiFi] Scan failed: " + str(e))
        return []
    finally:
        gc.collect()



def _process(context, aps):
    # --- Build set of currently-visible BSSIDs ---
    visible = set()
    ap_info = {}  # bssid -> ssid
    for ap in aps:
        try:
            bssid = _bssid_str(ap[1])
            visible.add(bssid)
            try:
                ssid = ap[0].decode('utf-8') if ap[0] else ''
            except Exception:
                ssid = ''
            ap_info[bssid] = ssid[:16]
        except Exception:
            pass

    # --- Decay entries not currently visible ---
    for entry in context.wifi_familiar:
        if entry['b'] not in visible:
            entry['n'] -= _DECAY_PER_SCAN
    for entry in context.wifi_recent:
        if entry['b'] not in visible:
            entry['n'] -= _DECAY_PER_SCAN

    # --- Prune fully-decayed entries ---
    context.wifi_familiar = [e for e in context.wifi_familiar if e['n'] > 0.0]
    context.wifi_recent   = [e for e in context.wifi_recent   if e['n'] > 0.0]

    # --- Fast-lookup indexes ---
    fam_idx    = {e['b']: e for e in context.wifi_familiar}
    recent_idx = {e['b']: e for e in context.wifi_recent}

    # --- Increment seen entries; add genuinely new ones to recent ---
    for bssid, ssid in ap_info.items():
        if bssid in fam_idx:
            fam_idx[bssid]['n'] += 1.0
        elif bssid in recent_idx:
            recent_idx[bssid]['n'] += 1.0
        else:
            # New network: add to recent, evicting the lowest-count entry if full
            new_entry = {'b': bssid, 's': ssid, 'n': 1.0}
            if len(context.wifi_recent) < _RECENT_MAX:
                context.wifi_recent.append(new_entry)
                recent_idx[bssid] = new_entry
            else:
                victim = min(context.wifi_recent, key=lambda e: e['n'])
                if victim['n'] < new_entry['n']:
                    context.wifi_recent.remove(victim)
                    del recent_idx[victim['b']]
                    context.wifi_recent.append(new_entry)
                    recent_idx[bssid] = new_entry

    # --- Promote: recent entries that have hit the threshold ---
    for candidate in [e for e in context.wifi_recent if e['n'] >= _PROMOTE_MIN]:
        if len(context.wifi_familiar) < _FAMILIAR_MAX:
            context.wifi_recent.remove(candidate)
            context.wifi_familiar.append(candidate)
            fam_idx[candidate['b']] = candidate
        else:
            weakest = min(context.wifi_familiar, key=lambda e: e['n'])
            if candidate['n'] > weakest['n']:
                # Swap: demote weakest to recent (if there's room), promote candidate
                context.wifi_familiar.remove(weakest)
                del fam_idx[weakest['b']]
                context.wifi_recent.remove(candidate)
                context.wifi_familiar.append(candidate)
                fam_idx[candidate['b']] = candidate
                # Only keep the demoted entry if recent has room
                if len(context.wifi_recent) < _RECENT_MAX:
                    context.wifi_recent.append(weakest)
                    recent_idx[weakest['b']] = weakest

    # --- Update location flag ---
    context.in_familiar_location = any(e['b'] in visible for e in context.wifi_familiar)
