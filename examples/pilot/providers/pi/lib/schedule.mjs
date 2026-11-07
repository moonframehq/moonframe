// When a scheduled task runs next. Tasks run while the app is open; one
// missed while it was closed waits for its next time.
const DAY = 86_400;

function at(date, time) {
	const [hours, minutes] = String(time || "09:00").split(":").map(Number);
	const moment = new Date(date);
	moment.setHours(hours || 0, minutes || 0, 0, 0);
	return moment;
}

export function nextRun(task, after) {
	const from = new Date(after * 1000);
	if (task.cadence === "hourly") {
		const next = new Date(from);
		next.setMinutes(Number(String(task.time).split(":")[1]) || 0, 0, 0);
		if (next <= from) next.setHours(next.getHours() + 1);
		return next.getTime() / 1000;
	}
	for (let days = 0; days < 8; days++) {
		const moment = at(new Date((after + days * DAY) * 1000), task.time);
		const weekday = moment.getDay();
		if (moment <= from) continue;
		if (task.cadence === "weekdays" && (weekday === 0 || weekday === 6)) continue;
		if (task.cadence === "weekly" && weekday !== (task.weekday ?? 1)) continue;
		return moment.getTime() / 1000;
	}
	return after + DAY;
}

// At launch: what came due while the app was closed waits for its next time.
export function skipMissed(tasks, now) {
	let moved = false;
	for (const task of tasks) {
		if (task.next && task.next <= now) {
			task.next = nextRun(task, now);
			moved = true;
		}
	}
	return moved;
}

const WEEKDAYS = ["Sundays", "Mondays", "Tuesdays", "Wednesdays", "Thursdays", "Fridays", "Saturdays"];

export function describe(task) {
	const time = task.time || "09:00";
	switch (task.cadence) {
		case "hourly":
			return `Every hour at :${time.split(":")[1] ?? "00"}`;
		case "weekdays":
			return `Weekdays at ${time}`;
		case "weekly":
			return `${WEEKDAYS[task.weekday ?? 1]} at ${time}`;
		default:
			return `Every day at ${time}`;
	}
}
