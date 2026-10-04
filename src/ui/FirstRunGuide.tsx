import { CheckIcon, PlusIcon } from "../Icons";
import { profileCopy, type Locale } from "../i18n";
import type { GhProfileStatus, Profile, RepositoryRecord } from "../types";
import { firstRunSteps } from "./profileStatus";

export function FirstRunGuide({ profiles, repositories, statuses, locale, onCreate, onConnect, onAdd }: {
  profiles: Profile[];
  repositories: RepositoryRecord[];
  statuses: Record<string, GhProfileStatus>;
  locale: Locale;
  onCreate: () => void;
  onConnect: () => void;
  onAdd: () => void;
}) {
  const copy = profileCopy[locale];
  const completed = firstRunSteps(profiles, repositories, statuses);
  const current = completed.findIndex((done) => !done);
  const actions = [onCreate, onConnect, onAdd];
  const labels = [copy.stepProfile, copy.stepGithub, copy.stepRepository];
  return <div className="ui-page ui-first-run">
    <div className="ui-page-heading"><h1>{copy.guideTitle}</h1></div>
    <p className="ui-muted">{copy.guideLead}</p>
    <ol className="ui-guide-steps">{labels.map((label, index) => <li className="ui-card" key={label}>
      <span className={`ui-guide-marker ${completed[index] ? "ui-success" : ""}`}>{completed[index] ? <CheckIcon /> : index + 1}</span>
      <div><h2>{label}</h2><p className="ui-muted">{completed[index] ? copy.completed : copy.stepHelp[index]}</p></div>
      {!completed[index] && index === current && <button className="ui-button ui-button--primary" type="button" onClick={actions[index]}><PlusIcon />{copy.stepAction[index]}</button>}
    </li>)}</ol>
  </div>;
}
