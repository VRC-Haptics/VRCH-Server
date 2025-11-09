import { useSettingsContext } from "../../context/SettingsProvider";
import { GitRepo } from "../../utils/commonClasses";
import { useState} from "react";

async function validateGitHubRepo(url: string): Promise<GitRepo | null> {
  const patterns = [
    /github\.com\/([^\/]+)\/([^\/\s]+)/,
    /^([^\/]+)\/([^\/\s]+)$/
  ];
  
  for (const pattern of patterns) {
    const match = url.trim().match(pattern);
    if (match) {
      const [, owner, name] = match;
      const cleanName = name.replace(/\.git$/, '');
      
      try {
        const response = await fetch(`https://api.github.com/repos/${owner}/${cleanName}`);
        if (response.ok) {
          return { owner, name: cleanName };
        }
      } catch (e) {
        console.error('Validation failed:', e);
      }
    }
  }
  return null;
}

export default function RepositorySettings() {
  const { repositories, updateRepositories } = useSettingsContext();
  const [inputUrl, setInputUrl] = useState('');
  const [error, setError] = useState('');
  const [validating, setValidating] = useState(false);

  const handleAdd = async () => {
    if (!inputUrl.trim()) return;
    
    setValidating(true);
    setError('');
    
    const repo = await validateGitHubRepo(inputUrl);
    if (repo) {
      const exists = repositories.some((r) => {
        r.owner === repo.owner && r.name === repo.name
    });
      if (!exists) {
        await updateRepositories([...repositories, repo]);
        setInputUrl('');
      } else {
        setError('Repository already added');
      }
    } else {
      setError('Invalid repository URL or repository not found');
    }
    
    setValidating(false);
  };

  const handleRemove = async (index: number) => {
    const repo = repositories[index];
    // Prevent removing default
    if (repo.owner === 'VRC-Haptics' && repo.name === 'VRCH-Firmware') {
      return;
    }
    await updateRepositories(repositories.filter((_, i) => i !== index));
  };

  const isDefault = (repo: GitRepo) => 
    repo.owner === 'VRC-Haptics' && repo.name === 'VRCH-Firmware';

  return (
    <div className="flex flex-col p-2 bg-base-200 rounded-md">
      <h3 className="font-semibold text-lg">Custom Repositories</h3>
      <h6 className="text-info text-sm p-1">
        Add GitHub repositories for firmware updates
      </h6>

      <div className="flex gap-2">
        <input
          type="text"
          placeholder="owner/repo or github.com/owner/repo"
          className="input input-primary input-sm flex-1"
          value={inputUrl}
          onChange={e => setInputUrl(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && handleAdd()}
        />
        <button 
          className="btn btn-primary btn-sm"
          onClick={handleAdd}
          disabled={validating}
        >
          {validating ? 'Checking...' : 'Add'}
        </button>
      </div>
      
      {error && <p className="text-error text-xs">{error}</p>}

      <div className="flex flex-col gap-1 mt-2">
        {repositories.map((repo, i) => (
          <div key={i} className="flex items-center justify-between bg-base-300 p-2 rounded">
            <span className="text-sm font-mono">
              {repo.owner}/{repo.name}
              {isDefault(repo) && (
                <span className="ml-2 text-xs opacity-60">(default)</span>
              )}
            </span>
            {!isDefault(repo) && (
              <button
                className="btn btn-ghost btn-xs"
                onClick={() => handleRemove(i)}
              >
                Remove
              </button>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}