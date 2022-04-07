import useSWR, { useSWRConfig } from "swr";
import { useEffect, useMemo, useRef, useState } from "react";
import { useLockFn } from "ahooks";
import { Virtuoso } from "react-virtuoso";
import { ApiType } from "../../services/types";
import { updateProxy } from "../../services/api";
import { getProfiles, patchProfile } from "../../services/cmds";
import useFilterProxy, { ProxySortType } from "./use-filter-proxy";
import delayManager from "../../services/delay";
import ProxyItem from "./proxy-item";
import ProxyHead from "./proxy-head";

interface Props {
  groupName: string;
  curProxy?: string;
  proxies: ApiType.ProxyItem[];
}

// this component will be used for DIRECT/GLOBAL
const ProxyGlobal = (props: Props) => {
  const { groupName, curProxy, proxies } = props;

  const { mutate } = useSWRConfig();
  const [now, setNow] = useState(curProxy || "DIRECT");

  const [showType, setShowType] = useState(true);
  const [sortType, setSortType] = useState<ProxySortType>(0);

  const [urlText, setUrlText] = useState("");
  const [filterText, setFilterText] = useState("");

  const virtuosoRef = useRef<any>();
  const filterProxies = useFilterProxy(proxies, groupName, filterText);

  const sortedProxies = useMemo(() => {
    if (sortType === 0) return filterProxies;

    const list = filterProxies.slice();

    if (sortType === 1) {
      list.sort((a, b) => a.name.localeCompare(b.name));
    } else {
      list.sort((a, b) => {
        const ad = delayManager.getDelay(a.name, groupName);
        const bd = delayManager.getDelay(b.name, groupName);

        if (ad === -1) return 1;
        if (bd === -1) return -1;

        return ad - bd;
      });
    }

    return list;
  }, [filterProxies, sortType, groupName]);

  const { data: profiles } = useSWR("getProfiles", getProfiles);

  const onChangeProxy = useLockFn(async (name: string) => {
    await updateProxy(groupName, name);
    setNow(name);

    if (groupName === "DIRECT") return;

    // update global selected
    const profile = profiles?.items?.find((p) => p.uid === profiles.current);
    if (!profile) return;
    if (!profile.selected) profile.selected = [];

    const index = profile.selected.findIndex((item) => item.name === groupName);
    if (index < 0) {
      profile.selected.unshift({ name: groupName, now: name });
    } else {
      profile.selected[index] = { name: groupName, now: name };
    }

    await patchProfile(profiles!.current!, { selected: profile.selected });
  });

  const onLocation = (smooth = true) => {
    const index = sortedProxies.findIndex((p) => p.name === now);

    if (index >= 0) {
      virtuosoRef.current?.scrollToIndex?.({
        index,
        align: "center",
        behavior: smooth ? "smooth" : "auto",
      });
    }
  };

  const onCheckAll = useLockFn(async () => {
    const names = sortedProxies.map((p) => p.name);

    await delayManager.checkListDelay(
      { names, groupName, skipNum: 8, maxTimeout: 600 },
      () => mutate("getProxies")
    );

    mutate("getProxies");
  });

  useEffect(() => onLocation(false), [groupName]);

  useEffect(() => {
    if (groupName === "DIRECT") setNow("DIRECT");
    else if (groupName === "GLOBAL") {
      if (profiles) {
        const current = profiles.current;
        const profile = profiles.items?.find((p) => p.uid === current);

        profile?.selected?.forEach((item) => {
          if (item.name === "GLOBAL") {
            if (item.now && item.now !== curProxy) {
              updateProxy("GLOBAL", item.now).then(() => setNow(item!.now!));
              mutate("getProxies");
            }
          }
        });
      }

      setNow(curProxy || "DIRECT");
    }
  }, [groupName, curProxy, profiles]);

  return (
    <>
      <ProxyHead
        sx={{ px: 3, my: 0.5, button: { mr: 0.5 } }}
        showType={showType}
        sortType={sortType}
        urlText={urlText}
        filterText={filterText}
        onLocation={onLocation}
        onCheckDelay={onCheckAll}
        onShowType={setShowType}
        onSortType={setSortType}
        onUrlText={setUrlText}
        onFilterText={setFilterText}
      />

      <Virtuoso
        ref={virtuosoRef}
        style={{ height: "calc(100% - 40px)" }}
        totalCount={sortedProxies.length}
        itemContent={(index) => (
          <ProxyItem
            groupName={groupName}
            proxy={sortedProxies[index]}
            selected={sortedProxies[index].name === now}
            showType={showType}
            onClick={onChangeProxy}
            sx={{ py: 0, px: 2 }}
          />
        )}
      />
    </>
  );
};

export default ProxyGlobal;
