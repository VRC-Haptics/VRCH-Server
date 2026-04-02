let Titles = "text-2xl font-bold padding-5 text-center";

export class GitRepo {
  owner: string;
  name: string;

  constructor(owner: string, name: string) {
    this.name = name;
    this.owner = owner;
  }
}